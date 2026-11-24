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
//! method takes the current time, so the decisions are unit-tested. [`acquire`] and [`observe`]
//! wrap one global instance for `trade::send_live`, the only place that reaches the network;
//! mock and cached replies (`devcache.rs`) never get there, so they neither count nor wait.
//! The `/data/*` endpoints send no limit headers and are never held back.

use anyhow::Result;
use reqwest::header::HeaderMap;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Waits up to this long are sat out (with a countdown in the overlay); longer ones fail at once
/// with "Trade site busy", since nobody wants to watch a minute-long spinner.
pub const MAX_WAIT: Duration = Duration::from_secs(10);

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

/// The endpoint a trade URL belongs to, as the limiter keys it: `trade/search`, `trade2/fetch`,
/// `trade/exchange`, … (the segments after `/api/`). Policies are learned per endpoint.
pub fn endpoint(path: &str) -> String {
    let mut it = path.split('/').filter(|s| !s.is_empty()).skip_while(|&s| s != "api").skip(1);
    match (it.next(), it.next()) {
        (Some(api), Some(ep)) => format!("{api}/{ep}"),
        _ => path.to_string(),
    }
}

#[derive(Debug, Default)]
struct Policy {
    rules: Vec<Rule>,
    /// When we sent (or the server counted) requests, oldest first; bounded by the largest
    /// `hits` and the longest period.
    log: Vec<Instant>,
    blocked_until: Option<Instant>,
}

impl Policy {
    fn count(&self, period: Duration, now: Instant) -> usize {
        self.log.iter().filter(|&&t| now.saturating_duration_since(t) < period).count()
    }

    /// How long until one more request fits every rule (one hit under each limit, as a margin for
    /// other tools on the same IP) and any restriction has ended.
    fn wait(&self, now: Instant) -> Duration {
        let mut wait = self.blocked_until.map_or(Duration::ZERO, |t| t.saturating_duration_since(now));
        for r in &self.rules {
            let allowed = r.allowed();
            let inside: Vec<&Instant> =
                self.log.iter().filter(|&&t| now.saturating_duration_since(t) < r.period).collect();
            if inside.len() >= allowed {
                // The oldest sends have to leave the window until only allowed - 1 remain.
                let oldest = *inside[inside.len() - allowed];
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
        let left = self.rules.iter().map(|r| r.allowed().saturating_sub(self.count(r.period, now)));
        left.min().map_or(u32::MAX, |n| u32::try_from(n).unwrap_or(u32::MAX))
    }

    /// Drops entries no rule looks at any more: older than the longest period, or beyond the
    /// largest hit count (only the most recent `hits` sends matter for any rule).
    fn trim(&mut self, now: Instant) {
        let longest = self.rules.iter().map(|r| r.period).max().unwrap_or_default();
        self.log.retain(|&t| now.saturating_duration_since(t) < longest);
        let cap = self.rules.iter().map(|r| r.hits as usize).max().unwrap_or(0);
        if self.log.len() > cap {
            self.log.drain(..self.log.len() - cap);
        }
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
    /// Asks to send a request to `endpoint` now. On `Go` the send is logged right away, so
    /// concurrent requests see each other before any reply arrives. Endpoints whose policy isn't
    /// known yet (the first request of a run) always go.
    pub fn try_send(&mut self, endpoint: &str, now: Instant) -> Decision {
        let Some(p) = self.endpoints.get(endpoint).and_then(|name| self.policies.get_mut(name)) else {
            return Decision::Go;
        };
        p.trim(now);
        match p.wait(now) {
            Duration::ZERO => {
                p.log.push(now);
                Decision::Go
            }
            w => Decision::Wait(w),
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
            let missing = (hits as usize).saturating_sub(p.count(period, now));
            let at = now.checked_sub(shorter).unwrap_or(now);
            p.log.extend(std::iter::repeat_n(at, missing));
            shorter = period;
        }
        p.log.sort();
        if let Some(r) = report.restricted {
            self.block(&report.policy, now + r);
        }
        if let Some(p) = self.policies.get_mut(&report.policy) {
            p.trim(now);
        }
    }

    /// A 429 from `endpoint`: block its policy until `retry_after` (default a minute) is over.
    pub fn rate_limited(&mut self, endpoint: &str, retry_after: Option<Duration>, now: Instant) {
        let Some(name) = self.endpoints.get(endpoint).cloned() else {
            // A 429 before we learned the policy: block the endpoint under its own name.
            self.endpoints.insert(endpoint.to_string(), endpoint.to_string());
            return self.rate_limited(endpoint, retry_after, now);
        };
        self.block(&name, now + retry_after.unwrap_or(Duration::from_secs(60)));
    }

    /// How many more requests to `endpoint` would go out now without waiting; None while its
    /// policy isn't known.
    pub fn headroom(&mut self, endpoint: &str, now: Instant) -> Option<u32> {
        let p = self.endpoints.get(endpoint).and_then(|name| self.policies.get_mut(name))?;
        p.trim(now);
        Some(p.headroom(now))
    }

    fn block(&mut self, policy: &str, until: Instant) {
        let p = self.policies.entry(policy.to_string()).or_default();
        p.blocked_until = p.blocked_until.max(Some(until));
    }

    /// True if nothing has been sent or learned (for tests: mock replies must leave it so).
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.endpoints.is_empty() && self.policies.values().all(|p| p.log.is_empty())
    }
}

static LIMITER: LazyLock<Mutex<Limiter>> = LazyLock::new(Mutex::default);

/// Shows the seconds left of a rate-limit wait in the UI (0 when done); set once by `main`.
static ON_WAIT: OnceLock<fn(u64)> = OnceLock::new();

pub fn on_wait(f: fn(u64)) {
    let _ = ON_WAIT.set(f);
}

fn show_wait(secs: u64) {
    if let Some(f) = ON_WAIT.get() {
        f(secs);
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

/// Shows the near-limit warning in the UI (empty to hide it); set once by `main`.
static ON_WARNING: OnceLock<fn(String)> = OnceLock::new();

pub fn on_warning(f: fn(String)) {
    let _ = ON_WARNING.set(f);
}

/// Updates the warning after a search request was let through or answered. Price checks are
/// searches (fetches have far looser limits), so only search endpoints drive it.
fn report(l: &mut Limiter, endpoint: &str, now: Instant) {
    if !endpoint.ends_with("/search") {
        return;
    }
    if let Some(f) = ON_WARNING.get() {
        f(warning(l.headroom(endpoint, now)));
    }
}

/// Clears the overlay's countdown however the wait ends (sent, gave up, or the task dropped).
struct WaitShown;
impl Drop for WaitShown {
    fn drop(&mut self) {
        show_wait(0);
    }
}

fn busy(wait: Duration) -> anyhow::Error {
    anyhow::anyhow!("Trade site busy, try again in {}s", wait.as_secs().max(1))
}

/// Waits until a request to `endpoint` is allowed and counts it, or fails with "Trade site busy"
/// when that would take longer than [`MAX_WAIT`] (in total, across retries).
pub async fn acquire(endpoint: &str) -> Result<()> {
    let start = Instant::now();
    let mut shown = None;
    loop {
        let now = Instant::now();
        let wait = {
            let mut l = LIMITER.lock().unwrap();
            match l.try_send(endpoint, now) {
                Decision::Go => {
                    report(&mut l, endpoint, now);
                    return Ok(());
                }
                Decision::Wait(w) => w,
            }
        };
        if now.duration_since(start) + wait > MAX_WAIT {
            return Err(busy(wait));
        }
        if shown.is_none() {
            log::info!("rate limit: waiting {:.1}s before {endpoint}", wait.as_secs_f64());
            shown = Some(WaitShown);
        }
        // Count down in whole seconds; re-check afterwards, as a concurrent request may have
        // taken the slot.
        show_wait(wait.as_secs_f64().ceil() as u64);
        tokio::time::sleep(wait.min(Duration::from_secs(1))).await;
    }
}

/// Feeds a reply's headers (and status) into the global limiter. Returns the error to report for
/// a 429.
pub fn observe(endpoint: &str, status: u16, headers: &HeaderMap) -> Option<anyhow::Error> {
    let now = Instant::now();
    let mut l = LIMITER.lock().unwrap();
    if let Some(r) = parse(headers) {
        log::debug!("rate limit {endpoint}: {r:?}");
        l.observe(endpoint, &r, now);
    }
    if status != 429 {
        report(&mut l, endpoint, now);
        return None;
    }
    let retry = retry_after(headers);
    l.rate_limited(endpoint, retry, now);
    report(&mut l, endpoint, now);
    let wait = retry.unwrap_or(Duration::from_secs(60));
    Some(anyhow::anyhow!("Rate limited by the trade API — retry in {}s", wait.as_secs()))
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
    fn endpoints_of_both_games() {
        assert_eq!(endpoint("/api/trade/search/Standard"), "trade/search");
        assert_eq!(endpoint("/api/trade2/search/poe2/Standard"), "trade2/search");
        assert_eq!(endpoint("/api/trade/fetch/a,b"), "trade/fetch");
        assert_eq!(endpoint("/api/trade2/exchange/poe2/Rise%20of%20the%20Abyssal"), "trade2/exchange");
        assert_eq!(endpoint("/somewhere"), "/somewhere");
    }

    fn learned(state: &str, now: Instant) -> Limiter {
        let mut l = Limiter::default();
        l.observe("trade/search", &parse(&search(state)).unwrap(), now);
        l
    }

    #[test]
    fn unknown_endpoint_goes() {
        let mut l = Limiter::default();
        assert_eq!(l.try_send("trade/search", Instant::now()), Decision::Go);
        assert!(l.is_empty(), "nothing to count against yet");
    }

    #[test]
    fn waits_one_hit_under_the_limit() {
        let t0 = Instant::now();
        // The first search (not logged by us: the policy was unknown) as the server counts it.
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        // 5 per 10 s with a one-hit margin: three more fit, a fifth waits.
        for i in 1..=3 {
            assert_eq!(l.try_send("trade/search", t0 + s(i)), Decision::Go, "send {i}");
        }
        // The oldest send (t0) leaves the 10 s window at t0 + 10.
        assert_eq!(l.try_send("trade/search", t0 + s(4)), Decision::Wait(s(6)));
        assert_eq!(l.try_send("trade/search", t0 + s(10)), Decision::Go);
    }

    #[test]
    fn longer_windows_count_too() {
        let t0 = Instant::now();
        // 14 of 15 per minute used, spread so the 10 s window is clear.
        let mut l = learned("0:10:0,14:60:0,14:300:0,14:21600:0", t0);
        // The server's 14 happened before the 10 s window: they leave the minute at t0 + 50.
        assert_eq!(l.try_send("trade/search", t0), Decision::Wait(s(50)));
        assert_eq!(l.try_send("trade/search", t0 + s(50)), Decision::Go);
    }

    #[test]
    fn server_count_wins_when_higher() {
        let t0 = Instant::now();
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        assert_eq!(l.try_send("trade/search", t0), Decision::Go);
        // Another tool on the IP: the server has seen 4 in the last 10 s, not our 2.
        l.observe("trade/search", &parse(&search("4:10:0,4:60:0,4:300:0,4:21600:0")).unwrap(), t0 + s(1));
        assert!(matches!(l.try_send("trade/search", t0 + s(1)), Decision::Wait(_)));
        // A lower count (our send still in flight) doesn't erase ours.
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        for _ in 0..3 {
            l.try_send("trade/search", t0);
        }
        l.observe("trade/search", &parse(&search("1:10:0,1:60:0,1:300:0,1:21600:0")).unwrap(), t0);
        assert!(matches!(l.try_send("trade/search", t0), Decision::Wait(_)));
    }

    #[test]
    fn restriction_from_state_blocks_the_policy() {
        let t0 = Instant::now();
        let mut l = learned("6:10:60,6:60:0,6:300:0,6:21600:0", t0);
        assert_eq!(l.try_send("trade/search", t0), Decision::Wait(s(60)));
        assert_eq!(l.try_send("trade/search", t0 + s(59)), Decision::Wait(s(1)));
        assert_eq!(l.try_send("trade/search", t0 + s(60)), Decision::Go);
    }

    #[test]
    fn policies_are_shared_between_games_and_separate_per_endpoint() {
        let t0 = Instant::now();
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        // PoE 2 search replies name the same policy.
        l.observe("trade2/search", &parse(&search("1:10:0,1:60:0,1:300:0,1:21600:0")).unwrap(), t0);
        for _ in 0..3 {
            assert_eq!(l.try_send("trade2/search", t0), Decision::Go);
        }
        assert!(matches!(l.try_send("trade/search", t0), Decision::Wait(_)), "PoE 1 search shares the count");
        assert_eq!(l.try_send("trade/fetch", t0), Decision::Go, "fetch has its own policy");
    }

    #[test]
    fn a_429_blocks_until_retry_after() {
        let t0 = Instant::now();
        let mut l = learned("1:10:0,1:60:0,1:300:0,1:21600:0", t0);
        l.rate_limited("trade/search", Some(s(30)), t0);
        assert_eq!(l.try_send("trade/search", t0 + s(1)), Decision::Wait(s(29)));
        assert_eq!(l.try_send("trade/search", t0 + s(30)), Decision::Go);
        // Before the policy is known: the endpoint itself is blocked, a minute by default.
        let mut l = Limiter::default();
        l.rate_limited("trade/exchange", None, t0);
        assert_eq!(l.try_send("trade/exchange", t0), Decision::Wait(s(60)));
        assert_eq!(l.try_send("trade/search", t0), Decision::Go);
    }

    #[test]
    fn log_stays_bounded() {
        let t0 = Instant::now();
        let mut l = learned("0:10:0,0:60:0,0:300:0,0:21600:0", t0);
        let mut t = t0;
        for _ in 0..2000 {
            t += s(30);
            l.try_send("trade/search", t);
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
        assert_eq!(l.headroom("trade/search", t0), None, "unknown until the first reply");
        l.observe("trade/search", &parse(&search("1:10:0,1:60:0,1:300:0,1:21600:0")).unwrap(), t0);
        // 5 per 10 s, one kept back, one used: three to go.
        assert_eq!(l.headroom("trade/search", t0), Some(3));
        l.try_send("trade/search", t0);
        assert_eq!(l.headroom("trade/search", t0), Some(2));
        l.try_send("trade/search", t0);
        l.try_send("trade/search", t0);
        assert_eq!(l.headroom("trade/search", t0), Some(0));
        assert_eq!(l.headroom("trade/search", t0 + s(10)), Some(4), "the window has passed");
    }

    #[test]
    fn headroom_takes_the_tightest_window_and_restrictions() {
        let t0 = Instant::now();
        // The 10 s window is clear, but 13 of the minute's 14 are used.
        let mut l = learned("0:10:0,13:60:0,13:300:0,13:21600:0", t0);
        assert_eq!(l.headroom("trade/search", t0), Some(1));
        l.rate_limited("trade/search", Some(s(30)), t0);
        assert_eq!(l.headroom("trade/search", t0), Some(0));
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
        assert_eq!(busy(Duration::from_millis(300)).to_string(), "Trade site busy, try again in 1s");
        assert_eq!(busy(s(45)).to_string(), "Trade site busy, try again in 45s");
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
