//! Pacing for the trade API (both games), so price checks in quick succession wait for GGG's
//! rate limits instead of running into them. Repeated violations lock the IP out for longer and
//! longer, so we never send a request the limits wouldn't allow.
//!
//! Every reply names its policy (`X-Rate-Limit-Policy`, one per endpoint: search, fetch, exchange;
//! PoE 1 and PoE 2 share them), the rule sets that apply (`X-Rate-Limit-Rules: Ip`), and per rule
//! set its limits and the server's current count:
//!
//! ```text
//! X-Rate-Limit-Ip:       5:10:60,15:60:300,30:300:1800   hits:period:penalty
//! X-Rate-Limit-Ip-State: 1:10:0,1:60:0,1:300:0           hits:period:active restriction (s)
//! ```
//!
//! [`Limiter`] keeps, per policy, those limits and a log of our own sends, corrected upwards from
//! the State header (requests from other tools on the same IP count too). It is plain data: every
//! method takes the current time, so the decisions are unit-tested, as are [`step`], [`reply`] and
//! [`warning_for`], the decisions of the async layer. [`acquire`], [`unsend`] and [`observe`] wrap
//! one global instance for `trade::send_live`, the only place that reaches the network; mock and
//! cached replies (`devcache.rs`) never get there, so they neither count nor wait.
//! The `/data/*` endpoints send no limit headers and are never held back.

use anyhow::Result;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Waits up to this long are sat out (with a countdown in the overlay); longer ones fail at once
/// with "Trade site busy", since nobody wants to watch a minute-long spinner.
pub const MAX_WAIT: Duration = Duration::from_secs(10);

/// How long a policy is blocked after a 429 that says neither how long nor what restriction.
const DEFAULT_BLOCK: Duration = Duration::from_secs(60);

/// One limit: at most `hits` requests per `period`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub hits: u32,
    pub period: Duration,
}

impl Rule {
    /// Requests we allow ourselves per window: one under the limit, as a margin for other tools
    /// on the same IP (but at least one).
    fn allowed(&self) -> usize {
        (self.hits.saturating_sub(1)).max(1) as usize
    }
}

/// What one reply says about its policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub policy: String,
    /// Limits of every rule set that applies (`Ip`, and `Account` when logged in), merged.
    pub rules: Vec<Rule>,
    /// The server's count per window: (hits, period).
    pub state: Vec<(u32, Duration)>,
    /// Longest active restriction in the State headers, if any.
    pub restricted: Option<Duration>,
}

/// Parses `a:b:c,a:b:c` into triples; malformed entries are skipped.
fn triples(s: &str) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
    s.split(',').filter_map(|t| {
        let mut it = t.trim().splitn(3, ':').map(|n| n.trim().parse::<u32>().ok());
        Some((it.next()??, it.next()??, it.next()??))
    })
}

/// Reads the rate-limit headers of a reply; None if it has none (the `/data/*` endpoints) or
/// they are unusable.
pub fn parse(headers: &HeaderMap) -> Option<Report> {
    let get = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let policy = get("x-rate-limit-policy")?.trim();
    if policy.is_empty() {
        return None;
    }
    let (mut rules, mut state, mut restricted) = (Vec::new(), Vec::new(), None);
    for set in get("x-rate-limit-rules")?.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let set = set.to_ascii_lowercase();
        if let Some(v) = get(&format!("x-rate-limit-{set}")) {
            rules.extend(
                triples(v)
                    .filter(|&(hits, period, _)| hits > 0 && period > 0)
                    .map(|(hits, period, _)| Rule { hits, period: secs(period) }),
            );
        }
        if let Some(v) = get(&format!("x-rate-limit-{set}-state")) {
            for (hits, period, active) in triples(v).filter(|&(_, period, _)| period > 0) {
                state.push((hits, secs(period)));
                if active > 0 {
                    restricted = restricted.max(Some(secs(active)));
                }
            }
        }
    }
    if rules.is_empty() {
        return None;
    }
    Some(Report { policy: policy.to_string(), rules, state, restricted })
}

/// `Retry-After` of a 429, in seconds.
pub fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    let s = headers.get("retry-after")?.to_str().ok()?;
    s.trim().parse::<u32>().ok().map(secs)
}

fn secs(n: u32) -> Duration {
    Duration::from_secs(u64::from(n))
}

/// The endpoint a trade URL belongs to, as the limiter keys it: `search`, `fetch`, `exchange`,
/// `data` (the segment after `/api/trade/` or `/api/trade2/`). The game is left out on purpose:
/// both trade sites share each policy, so a PoE 2 search counts against the PoE 1 searches.
pub fn endpoint(path: &str) -> String {
    let mut it = path.split('/').filter(|s| !s.is_empty()).skip_while(|&s| s != "api").skip(2);
    match it.next() {
        Some(ep) => ep.to_string(),
        None => path.to_string(),
    }
}

#[derive(Debug, Default)]
struct Policy {
    rules: Vec<Rule>,
    /// When we sent (or the server counted) requests, sorted oldest first; bounded by the largest
    /// `hits` and the longest period.
    log: Vec<Instant>,
    blocked_until: Option<Instant>,
}

impl Policy {
    /// The logged requests inside the window of length `period` ending `now`.
    fn window(&self, period: Duration, now: Instant) -> &[Instant] {
        let start = self.log.partition_point(|&t| now.saturating_duration_since(t) >= period);
        &self.log[start..]
    }

    fn insert(&mut self, t: Instant) {
        let at = self.log.partition_point(|&x| x <= t);
        self.log.insert(at, t);
    }

    /// How long until one more request fits every rule (one hit under each limit, as a margin for
    /// other tools on the same IP) and any restriction has ended.
    fn wait(&self, now: Instant) -> Duration {
        let mut wait = self.blocked_until.map_or(Duration::ZERO, |t| t.saturating_duration_since(now));
        for r in &self.rules {
            let (inside, allowed) = (self.window(r.period, now), r.allowed());
            if inside.len() >= allowed {
                // The oldest sends have to leave the window until only allowed - 1 remain.
                let oldest = inside[inside.len() - allowed];
                wait = wait.max((oldest + r.period).saturating_duration_since(now));
            }
        }
        wait
    }

    /// How many more requests go out right now without waiting, under every rule.
    fn headroom(&self, now: Instant) -> u32 {
        if self.blocked_until.is_some_and(|t| t > now) {
            return 0;
        }
        let left = self.rules.iter().map(|r| r.allowed().saturating_sub(self.window(r.period, now).len()));
        left.min().map_or(u32::MAX, |n| u32::try_from(n).unwrap_or(u32::MAX))
    }

    /// The soonest the headroom can grow: a restriction ends or a request leaves a window.
    fn next_change(&self, now: Instant) -> Option<Duration> {
        if let Some(t) = self.blocked_until.filter(|&t| t > now) {
            return Some(t - now);
        }
        self.rules
            .iter()
            .filter_map(|r| self.window(r.period, now).first().map(|&t| (t + r.period).saturating_duration_since(now)))
            .min()
    }

    /// Drops entries no rule looks at any more: older than the longest period, or beyond the
    /// largest hit count (only the most recent `hits` sends matter for any rule).
    fn trim(&mut self, now: Instant) {
        let longest = self.rules.iter().map(|r| r.period).max().unwrap_or_default();
        let start = self.log.len() - self.window(longest, now).len();
        let cap = self.rules.iter().map(|r| r.hits as usize).max().unwrap_or(0);
        self.log.drain(..start.max(self.log.len().saturating_sub(cap)));
        if self.blocked_until.is_some_and(|t| t <= now) {
            self.blocked_until = None;
        }
    }
}

/// The decision before a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Send now (the send is already counted).
    Go,
    /// Not yet: try again after this long.
    Wait(Duration),
}

/// Rate-limit bookkeeping for all trade endpoints. Plain data; the caller supplies the time.
#[derive(Debug, Default)]
pub struct Limiter {
    /// Endpoint (see [`endpoint`]) -> policy name, learned from replies.
    endpoints: HashMap<String, String>,
    policies: HashMap<String, Policy>,
}

impl Limiter {
    fn policy(&mut self, endpoint: &str) -> Option<&mut Policy> {
        self.endpoints.get(endpoint).and_then(|name| self.policies.get_mut(name))
    }

    /// Asks to send a request to `endpoint` now. On `Go` the send is logged right away, so
    /// concurrent requests see each other before any reply arrives. Endpoints whose policy isn't
    /// known yet (the first request of a run) always go.
    pub fn try_send(&mut self, endpoint: &str, now: Instant) -> Decision {
        let Some(p) = self.policy(endpoint) else {
            return Decision::Go;
        };
        p.trim(now);
        match p.wait(now) {
            Duration::ZERO => {
                p.insert(now);
                Decision::Go
            }
            w => Decision::Wait(w),
        }
    }

    /// Takes back a send logged at `at` that never reached the server (connection failed).
    pub fn unsend(&mut self, endpoint: &str, at: Instant) {
        if let Some(p) = self.policy(endpoint)
            && let Some(i) = p.log.iter().rposition(|&t| t == at)
        {
            p.log.remove(i);
        }
    }

    /// Takes in what a reply from `endpoint` said. The server's counts win where they are higher
    /// than ours (other tools on the same IP, or requests from before this run); lower counts are
    /// ignored, since our own requests still in flight may not be in them yet.
    pub fn observe(&mut self, endpoint: &str, report: &Report, now: Instant) {
        self.endpoints.insert(endpoint.to_string(), report.policy.clone());
        let p = self.policies.entry(report.policy.clone()).or_default();
        p.rules = report.rules.clone();
        // Shortest window first: requests the server counts in a longer window but not in the
        // shorter one happened before the shorter one began, so they go just outside it (the
        // latest they can have been, which keeps the estimate on the safe side).
        let mut state = report.state.clone();
        state.sort_by_key(|&(_, period)| period);
        let mut shorter = Duration::ZERO;
        for (hits, period) in state {
            let missing = (hits as usize).saturating_sub(p.window(period, now).len());
            let at = now.checked_sub(shorter).unwrap_or(now);
            p.log.extend(std::iter::repeat_n(at, missing));
            shorter = period;
        }
        p.log.sort();
        if let Some(r) = report.restricted {
            p.blocked_until = p.blocked_until.max(Some(now + r));
        }
        p.trim(now);
    }

    /// A 429 from `endpoint`: block its policy until `retry_after` (default a minute) is over. A
    /// 429 before the policy is known blocks the endpoint under its own name.
    pub fn rate_limited(&mut self, endpoint: &str, retry_after: Option<Duration>, now: Instant) {
        let name = self.endpoints.entry(endpoint.to_string()).or_insert_with(|| endpoint.to_string()).clone();
        let p = self.policies.entry(name).or_default();
        p.blocked_until = p.blocked_until.max(Some(now + retry_after.unwrap_or(DEFAULT_BLOCK)));
    }

    /// How many more requests to `endpoint` would go out now without waiting; None while its
    /// policy isn't known.
    pub fn headroom(&mut self, endpoint: &str, now: Instant) -> Option<u32> {
        let p = self.policy(endpoint)?;
        p.trim(now);
        Some(p.headroom(now))
    }

    /// The soonest [`Limiter::headroom`] of `endpoint` can grow, if it can.
    pub fn next_change(&mut self, endpoint: &str, now: Instant) -> Option<Duration> {
        self.policy(endpoint)?.next_change(now)
    }

    /// How long `endpoint` is blocked by a restriction or 429.
    pub fn blocked_for(&mut self, endpoint: &str, now: Instant) -> Duration {
        self.policy(endpoint)
            .and_then(|p| p.blocked_until)
            .map_or(Duration::ZERO, |t| t.saturating_duration_since(now))
    }

    /// True if nothing has been sent or learned (for tests: mock replies must leave it so).
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.endpoints.is_empty() && self.policies.values().all(|p| p.log.is_empty())
    }
}

/// Warn in the overlay once this few searches are left before the limit makes us wait.
pub const WARN_AT: u32 = 2;

/// The overlay footer's near-limit warning for `left` searches to go; empty when there's room.
pub fn warning(left: Option<u32>) -> String {
    match left {
        Some(0) => "Next search waits for the rate limit".into(),
        Some(1) => "1 search left before the rate limit".into(),
        Some(n) if n <= WARN_AT => format!("{n} searches left before the rate limit"),
        _ => String::new(),
    }
}

/// The footer warning after a request to `endpoint`, or None if that endpoint doesn't drive it.
/// Price checks are searches (fetches have far looser limits), so only searches do.
pub fn warning_for(l: &mut Limiter, endpoint: &str, now: Instant) -> Option<String> {
    (endpoint == "search").then(|| warning(l.headroom(endpoint, now)))
}

/// Whether a wait for `endpoint` belongs in the overlay's loading row: the price search's own
/// requests. The bulk exchange runs beside it and waits silently.
pub fn shows_wait(endpoint: &str) -> bool {
    matches!(endpoint, "search" | "fetch")
}

/// One round of [`acquire`], after `waited` so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Go,
    Wait(Duration),
    /// The wait would run past [`MAX_WAIT`]: give up with "Trade site busy".
    Busy(Duration),
}

pub fn step(l: &mut Limiter, endpoint: &str, now: Instant, waited: Duration) -> Step {
    match l.try_send(endpoint, now) {
        Decision::Go => Step::Go,
        Decision::Wait(w) if waited + w > MAX_WAIT => Step::Busy(w),
        Decision::Wait(w) => Step::Wait(w),
    }
}

fn busy(wait: Duration) -> String {
    format!("Trade site busy, try again in {}s", wait.as_secs().max(1))
}

/// Takes in a reply's status and headers; returns the message to show for a 429. Its wait is
/// what the limiter will actually hold back for (Retry-After or a State restriction, whichever is
/// longer), and only a vague "a minute" when the reply said neither.
pub fn reply(l: &mut Limiter, endpoint: &str, status: u16, headers: &HeaderMap, now: Instant) -> Option<String> {
    let report = parse(headers);
    if let Some(r) = &report {
        log::debug!("rate limit {endpoint}: {r:?}");
        l.observe(endpoint, r, now);
    }
    if status != 429 {
        return None;
    }
    let retry = retry_after(headers);
    l.rate_limited(endpoint, retry, now);
    if retry.is_none() && report.and_then(|r| r.restricted).is_none() {
        return Some("Rate limited by the trade API — try again in a minute".into());
    }
    let wait = l.blocked_for(endpoint, now).as_secs().max(1);
    Some(format!("Rate limited by the trade API — retry in {wait}s"))
}

// ── Global instance, for `trade::send_live` ──────────────────────────────────

static LIMITER: LazyLock<Mutex<Limiter>> = LazyLock::new(Mutex::default);

/// UI hooks, set once by `main`: the loading row's countdown (0 to hide) and the footer warning
/// (empty to hide).
static ON_WAIT: OnceLock<fn(u64)> = OnceLock::new();
static ON_WARNING: OnceLock<fn(String)> = OnceLock::new();

pub fn on_wait(f: fn(u64)) {
    let _ = ON_WAIT.set(f);
}

pub fn on_warning(f: fn(String)) {
    let _ = ON_WARNING.set(f);
}

/// Which waiter owns the countdown (0: none), so one ending doesn't clear another's.
static WAIT_OWNER: AtomicU64 = AtomicU64::new(0);
static WAIT_IDS: AtomicU64 = AtomicU64::new(1);

/// A waiter's countdown in the loading row; cleared on drop (sent, gave up, or the search was
/// cancelled) unless another waiter has taken it over since.
struct Countdown(u64);

impl Countdown {
    fn new() -> Self {
        Countdown(WAIT_IDS.fetch_add(1, Ordering::Relaxed))
    }

    fn show(&self, wait: Duration) {
        WAIT_OWNER.store(self.0, Ordering::SeqCst);
        if let Some(f) = ON_WAIT.get() {
            f(wait.as_secs_f64().ceil() as u64);
        }
    }
}

impl Drop for Countdown {
    fn drop(&mut self) {
        if WAIT_OWNER.compare_exchange(self.0, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok()
            && let Some(f) = ON_WAIT.get()
        {
            f(0);
        }
    }
}

/// Bumped on every warning update; a pending refresh only runs if it's still the latest.
static WARNING_GEN: AtomicU64 = AtomicU64::new(0);

/// Shows the search warning as of `now`. While it shows, one refresh is scheduled for when the
/// headroom can next grow (at most a minute out), so the warning clears by itself once the
/// window has room again. Each refresh task ends after its sleep.
fn publish_warning(l: &mut Limiter, now: Instant) {
    let Some(text) = warning_for(l, "search", now) else { return };
    let generation = WARNING_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let next = if text.is_empty() { None } else { l.next_change("search", now) };
    if let Some(f) = ON_WARNING.get() {
        f(text);
    }
    if let (Some(d), Ok(rt)) = (next, tokio::runtime::Handle::try_current()) {
        rt.spawn(async move {
            tokio::time::sleep(d.min(Duration::from_secs(60)) + Duration::from_millis(100)).await;
            if WARNING_GEN.load(Ordering::SeqCst) == generation {
                publish_warning(&mut LIMITER.lock().unwrap(), Instant::now());
            }
        });
    }
}

/// Re-evaluates the footer warning now, e.g. when the overlay opens for a new item.
pub fn refresh_warning() {
    publish_warning(&mut LIMITER.lock().unwrap(), Instant::now());
}

/// Waits until a request to `endpoint` is allowed and counts it, or fails with "Trade site busy"
/// when that would take longer than [`MAX_WAIT`] in total. Returns when the send was logged, for
/// [`unsend`]. Dropping the future (a cancelled search) while it waits sends nothing.
pub async fn acquire(endpoint: &str) -> Result<Instant> {
    let start = Instant::now();
    let (mut countdown, mut logged) = (None, false);
    loop {
        let now = Instant::now();
        let wait = {
            let mut l = LIMITER.lock().unwrap();
            match step(&mut l, endpoint, now, now.duration_since(start)) {
                Step::Go => {
                    if endpoint == "search" {
                        publish_warning(&mut l, now);
                    }
                    return Ok(now);
                }
                Step::Busy(w) => anyhow::bail!(busy(w)),
                Step::Wait(w) => w,
            }
        };
        if !logged {
            log::info!("rate limit: waiting {:.1}s before {endpoint}", wait.as_secs_f64());
            logged = true;
        }
        if shows_wait(endpoint) {
            countdown.get_or_insert_with(Countdown::new).show(wait);
        }
        // Count down in whole seconds; re-check afterwards, as a concurrent request may have
        // taken the slot.
        tokio::time::sleep(wait.min(Duration::from_secs(1))).await;
    }
}

/// Takes back a send that never reached the server (see [`Limiter::unsend`]).
pub fn unsend(endpoint: &str, at: Instant) {
    LIMITER.lock().unwrap().unsend(endpoint, at);
}

/// Feeds a reply into the global limiter; returns the error to report for a 429.
pub fn observe(endpoint: &str, status: u16, headers: &HeaderMap) -> Option<anyhow::Error> {
    let now = Instant::now();
    let mut l = LIMITER.lock().unwrap();
    let err = reply(&mut l, endpoint, status, headers, now);
    if endpoint == "search" {
        publish_warning(&mut l, now);
    }
    err.map(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Game;
    use reqwest::header::{HeaderName, HeaderValue};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        pairs
            .iter()
            .map(|(k, v)| (HeaderName::from_bytes(k.as_bytes()).unwrap(), HeaderValue::from_str(v).unwrap()))
            .collect()
    }

    /// Headers of a real PoE 1 search reply (2026-11).
    fn search(state: &str) -> HeaderMap {
        headers(&[
            ("X-Rate-Limit-Policy", "trade-search-request-limit"),
            ("X-Rate-Limit-Rules", "Ip"),
            ("X-Rate-Limit-Ip", "5:10:60,15:60:300,30:300:1800,600:21600:3600"),
            ("X-Rate-Limit-Ip-State", state),
        ])
    }

    fn rule(hits: u32, period: u64) -> Rule {
        Rule { hits, period: Duration::from_secs(period) }
    }

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn parses_real_headers() {
        let r = parse(&search("1:10:0,1:60:0,1:300:0,1:21600:0")).unwrap();
        assert_eq!(r.policy, "trade-search-request-limit");
        assert_eq!(r.rules, vec![rule(5, 10), rule(15, 60), rule(30, 300), rule(600, 21600)]);
        assert_eq!(r.state, vec![(1, s(10)), (1, s(60)), (1, s(300)), (1, s(21600))]);
        assert_eq!(r.restricted, None);
    }

    #[test]
    fn parses_several_rule_sets_and_restrictions() {
        let r = parse(&headers(&[
            ("x-rate-limit-policy", "trade-fetch-request-limit"),
            ("x-rate-limit-rules", "Account, Ip"),
            ("x-rate-limit-account", "3:5:60"),
            ("x-rate-limit-account-state", "3:5:42"),
            ("x-rate-limit-ip", "12:4:10,16:12:300"),
            ("x-rate-limit-ip-state", "1:4:0,2:12:7"),
        ]))
        .unwrap();
        assert_eq!(r.rules, vec![rule(3, 5), rule(12, 4), rule(16, 12)]);
        assert_eq!(r.restricted, Some(s(42)));
    }

    #[test]
    fn ignores_missing_or_odd_headers() {
        assert_eq!(parse(&HeaderMap::new()), None, "/data replies have none");
        assert_eq!(parse(&headers(&[("x-rate-limit-policy", "p")])), None, "no rules header");
        assert_eq!(
            parse(&headers(&[("x-rate-limit-policy", "p"), ("x-rate-limit-rules", "Ip"), ("x-rate-limit-ip", "junk")])),
            None
        );
        let r = parse(&headers(&[
            ("x-rate-limit-policy", "p"),
            ("x-rate-limit-rules", "Ip,,Missing"),
            ("x-rate-limit-ip", "5:10:60,x:1:2,0:10:5,3:0:1,8:20"),
            ("x-rate-limit-ip-state", "garbage"),
        ]))
        .unwrap();
        assert_eq!(r.rules, vec![rule(5, 10)], "bad, zero and short triples are skipped");
        assert!(r.state.is_empty());
    }

    #[test]
    fn retry_after_header() {
        assert_eq!(retry_after(&headers(&[("retry-after", "60")])), Some(s(60)));
        assert_eq!(retry_after(&headers(&[("retry-after", "soon")])), None);
        assert_eq!(retry_after(&HeaderMap::new()), None);
    }

    #[test]
    fn endpoints_are_the_same_for_both_games() {
        assert_eq!(endpoint("/api/trade/search/Standard"), "search");
        assert_eq!(endpoint("/api/trade2/search/poe2/Standard"), "search");
        assert_eq!(endpoint("/api/trade/fetch/a,b"), "fetch");
        assert_eq!(endpoint("/api/trade2/exchange/poe2/Rise%20of%20the%20Abyssal"), "exchange");
        assert_eq!(endpoint("/api/trade2/data/stats"), "data");
        assert_eq!(endpoint("/somewhere"), "/somewhere");
    }

    fn learned(state: &str, now: Instant) -> Limiter {
        let mut l = Limiter::default();
        l.observe("search", &parse(&search(state)).unwrap(), now);
        l
    }

    #[test]
    fn unknown_endpoint_goes() {
        let mut l = Limiter::default();
        assert_eq!(l.try_send("search", Instant::now()), Decision::Go);
        assert!(l.is_empty(), "nothing to count against yet");
    }

    #[test]
    fn waits_one_hit_under_the_limit() {
        let t0 = Instant::now();
        // The first search (not logged by us: the policy was unknown) as the server counts it.
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        // 5 per 10 s with a one-hit margin: three more fit, a fifth waits.
        for i in 1..=3 {
            assert_eq!(l.try_send("search", t0 + s(i)), Decision::Go, "send {i}");
        }
        // The oldest send (t0) leaves the 10 s window at t0 + 10.
        assert_eq!(l.try_send("search", t0 + s(4)), Decision::Wait(s(6)));
        assert_eq!(l.try_send("search", t0 + s(10)), Decision::Go);
    }

    #[test]
    fn longer_windows_count_too() {
        let t0 = Instant::now();
        // 14 of 15 per minute used, spread so the 10 s window is clear.
        let mut l = learned("0:10:0,14:60:0,14:300:0,14:21600:0", t0);
        // The server's 14 happened before the 10 s window: they leave the minute at t0 + 50.
        assert_eq!(l.try_send("search", t0), Decision::Wait(s(50)));
        assert_eq!(l.try_send("search", t0 + s(50)), Decision::Go);
    }

    #[test]
    fn server_count_wins_when_higher() {
        let t0 = Instant::now();
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        assert_eq!(l.try_send("search", t0), Decision::Go);
        // Another tool on the IP: the server has seen 4 in the last 10 s, not our 2.
        l.observe("search", &parse(&search("4:10:0,4:60:0,4:300:0,4:21600:0")).unwrap(), t0 + s(1));
        assert!(matches!(l.try_send("search", t0 + s(1)), Decision::Wait(_)));
        // A lower count (our send still in flight) doesn't erase ours.
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        for _ in 0..3 {
            l.try_send("search", t0);
        }
        l.observe("search", &parse(&search("1:10:0,1:60:0,1:300:0,1:21600:0")).unwrap(), t0);
        assert!(matches!(l.try_send("search", t0), Decision::Wait(_)));
    }

    #[test]
    fn restriction_from_state_blocks_the_policy() {
        let t0 = Instant::now();
        let mut l = learned("6:10:60,6:60:0,6:300:0,6:21600:0", t0);
        assert_eq!(l.try_send("search", t0), Decision::Wait(s(60)));
        assert_eq!(l.try_send("search", t0 + s(59)), Decision::Wait(s(1)));
        assert_eq!(l.try_send("search", t0 + s(60)), Decision::Go);
    }

    #[test]
    fn games_share_a_policy_and_endpoints_have_their_own() {
        let t0 = Instant::now();
        // Learned from a PoE 1 search; the first PoE 2 search is already held to it.
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        let poe2 = endpoint("/api/trade2/search/poe2/Standard");
        for _ in 0..3 {
            assert_eq!(l.try_send(&poe2, t0), Decision::Go);
        }
        assert!(matches!(l.try_send(&poe2, t0), Decision::Wait(_)));
        assert_eq!(l.try_send("fetch", t0), Decision::Go, "fetch has its own policy");
    }

    #[test]
    fn a_429_blocks_until_retry_after() {
        let t0 = Instant::now();
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        l.rate_limited("search", Some(s(30)), t0);
        assert_eq!(l.try_send("search", t0 + s(1)), Decision::Wait(s(29)));
        assert_eq!(l.try_send("search", t0 + s(30)), Decision::Go);
        // Before the policy is known: the endpoint itself is blocked, a minute by default.
        let mut l = Limiter::default();
        l.rate_limited("exchange", None, t0);
        assert_eq!(l.try_send("exchange", t0), Decision::Wait(s(60)));
        assert_eq!(l.try_send("search", t0), Decision::Go);
    }

    #[test]
    fn log_stays_bounded() {
        let t0 = Instant::now();
        let mut l = learned("0:10:0,0:60:0,0:300:0,0:21600:0", t0);
        let mut t = t0;
        for _ in 0..2000 {
            t += s(30);
            l.try_send("search", t);
        }
        let p = &l.policies["trade-search-request-limit"];
        assert!(p.log.len() <= 600, "{}", p.log.len());
    }

    #[test]
    fn single_hit_limit_still_allows_one() {
        let t0 = Instant::now();
        let mut l = Limiter::default();
        let r = Report { policy: "p".into(), rules: vec![rule(1, 5)], state: vec![], restricted: None };
        l.observe("x", &r, t0);
        assert_eq!(l.try_send("x", t0), Decision::Go);
        assert_eq!(l.try_send("x", t0 + s(1)), Decision::Wait(s(4)));
    }

    #[test]
    fn headroom_counts_down_to_the_limit() {
        let t0 = Instant::now();
        let mut l = Limiter::default();
        assert_eq!(l.headroom("search", t0), None, "unknown until the first reply");
        l.observe("search", &parse(&search("1:10:0,1:60:0,1:300:0,1:21600:0")).unwrap(), t0);
        // 5 per 10 s, one kept back, one used: three to go.
        assert_eq!(l.headroom("search", t0), Some(3));
        l.try_send("search", t0);
        assert_eq!(l.headroom("search", t0), Some(2));
        l.try_send("search", t0);
        l.try_send("search", t0);
        assert_eq!(l.headroom("search", t0), Some(0));
        assert_eq!(l.headroom("search", t0 + s(10)), Some(4), "the window has passed");
    }

    #[test]
    fn headroom_takes_the_tightest_window_and_restrictions() {
        let t0 = Instant::now();
        // The 10 s window is clear, but 13 of the minute's 14 are used.
        let mut l = learned("0:10:0,13:60:0,13:300:0,13:21600:0", t0);
        assert_eq!(l.headroom("search", t0), Some(1));
        l.rate_limited("search", Some(s(30)), t0);
        assert_eq!(l.headroom("search", t0), Some(0));
    }

    #[test]
    fn warning_only_near_the_limit() {
        assert_eq!(warning(None), "");
        assert_eq!(warning(Some(3)), "");
        assert_eq!(warning(Some(u32::MAX)), "");
        assert_eq!(warning(Some(2)), "2 searches left before the rate limit");
        assert_eq!(warning(Some(1)), "1 search left before the rate limit");
        assert_eq!(warning(Some(0)), "Next search waits for the rate limit");
    }

    #[test]
    fn busy_message_rounds_up_to_a_second() {
        assert_eq!(busy(Duration::from_millis(300)), "Trade site busy, try again in 1s");
        assert_eq!(busy(s(45)), "Trade site busy, try again in 45s");
    }

    #[test]
    fn sends_logged_out_of_order_stay_sorted() {
        // Two requests take their time, then lock in the opposite order.
        let t0 = Instant::now();
        let mut l = learned("0:10:0,0:60:0,0:300:0,0:21600:0", t0);
        assert_eq!(l.try_send("search", t0 + s(2)), Decision::Go);
        assert_eq!(l.try_send("search", t0 + s(1)), Decision::Go);
        let p = &l.policies["trade-search-request-limit"];
        assert!(p.log.is_sorted());
    }

    #[test]
    fn unsend_takes_back_a_failed_send() {
        let t0 = Instant::now();
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        l.try_send("search", t0 + s(1));
        assert_eq!(l.headroom("search", t0 + s(1)), Some(2));
        l.unsend("search", t0 + s(1));
        assert_eq!(l.headroom("search", t0 + s(1)), Some(3));
        l.unsend("search", t0 + s(5));
        assert_eq!(l.headroom("search", t0 + s(1)), Some(3), "nothing logged then: no change");
        l.unsend("fetch", t0);
    }

    #[test]
    fn next_change_is_when_a_request_leaves_a_window() {
        let t0 = Instant::now();
        let mut l = Limiter::default();
        assert_eq!(l.next_change("search", t0), None);
        l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        assert_eq!(l.next_change("search", t0 + s(3)), Some(s(7)));
        l.rate_limited("search", Some(s(30)), t0);
        assert_eq!(l.next_change("search", t0), Some(s(30)), "a restriction first");
    }

    #[test]
    fn step_gives_up_past_max_wait() {
        let t0 = Instant::now();
        let mut l = learned("4:10:0,4:60:0,4:300:0,4:21600:0", t0);
        assert_eq!(step(&mut l, "search", t0 + s(1), Duration::ZERO), Step::Wait(s(9)));
        assert_eq!(step(&mut l, "search", t0 + s(1), s(2)), Step::Busy(s(9)), "2 s already waited");
        assert_eq!(step(&mut l, "search", t0 + s(10), s(9)), Step::Go);
        l.rate_limited("search", Some(s(300)), t0);
        assert_eq!(step(&mut l, "search", t0 + s(11), Duration::ZERO), Step::Busy(s(289)));
        assert_eq!(step(&mut l, "exchange", t0, Duration::ZERO), Step::Go, "unknown: goes");
    }

    #[test]
    fn reply_reports_how_long_we_really_hold_back() {
        let t0 = Instant::now();
        let mut l = Limiter::default();
        assert_eq!(reply(&mut l, "search", 200, &search("1:10:0,1:60:0,1:300:0,1:21600:0"), t0), None);
        assert_eq!(l.headroom("search", t0), Some(3), "a success is still taken in");
        // A 429 whose State says 300 s and Retry-After says less: the longer one is shown.
        let mut h = search("6:10:0,16:60:300,16:300:0,16:21600:0");
        h.insert("retry-after", HeaderValue::from_static("60"));
        let msg = reply(&mut l, "search", 429, &h, t0).unwrap();
        assert_eq!(msg, "Rate limited by the trade API — retry in 300s");
        assert_eq!(l.blocked_for("search", t0), s(300));
        // A bare 429 says nothing about how long: no made-up figure.
        let mut l = Limiter::default();
        let msg = reply(&mut l, "fetch", 429, &HeaderMap::new(), t0).unwrap();
        assert_eq!(msg, "Rate limited by the trade API — try again in a minute");
        assert_eq!(l.blocked_for("fetch", t0), s(60));
        assert_eq!(reply(&mut l, "data", 404, &HeaderMap::new(), t0), None);
    }

    #[test]
    fn only_searches_drive_the_warning_and_countdown() {
        let t0 = Instant::now();
        let mut l = learned("3:10:0,3:60:0,3:300:0,3:21600:0", t0);
        assert_eq!(warning_for(&mut l, "search", t0).as_deref(), Some("1 search left before the rate limit"));
        assert_eq!(warning_for(&mut l, "search", t0 + s(10)).as_deref(), Some(""), "window passed: hidden");
        assert_eq!(warning_for(&mut l, "fetch", t0), None);
        assert_eq!(warning_for(&mut l, "exchange", t0), None);
        assert!(shows_wait("search") && shows_wait("fetch"));
        assert!(!shows_wait("exchange") && !shows_wait("data"));
    }

    #[test]
    fn mock_replies_are_not_counted() {
        // Tests always use mock.rs; a whole price check must leave the global limiter untouched.
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        for (file, game) in [("dev/boots.txt", Game::Poe1), ("dev/boots-poe2.txt", Game::Poe2)] {
            let cfg = crate::config::Config { game, ..Default::default() };
            let item = crate::item::parse(&std::fs::read_to_string(file).unwrap()).unwrap();
            rt.block_on(async {
                let spec = crate::trade::prepare(&item, &cfg).await.unwrap();
                crate::trade::search(&spec, &cfg).await.unwrap();
            });
        }
        assert!(LIMITER.lock().unwrap().is_empty());
    }
}
