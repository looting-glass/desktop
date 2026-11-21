//! Dev aids for API responses. `LOOTINGGLASS_MOCK=1` answers from canned data (see `mock.rs`);
//! `LOOTINGGLASS_HTTP_CACHE=<dir>` records every trade API / poe.ninja JSON response in
//! `<dir>` and replays it on later runs, so UI work (`LOOTINGGLASS_DEMO`, `LOOTINGGLASS_SNAPSHOT`)
//! doesn't hit the APIs or their rate limits. Delete the directory to fetch fresh data.

use anyhow::Result;
use serde_json::Value;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;

fn dir() -> Option<PathBuf> {
    std::env::var_os("LOOTINGGLASS_HTTP_CACHE").map(PathBuf::from)
}

/// Cache file for a request: hash of method, URL and body (headers don't matter).
fn file(dir: &std::path::Path, req: &reqwest::RequestBuilder) -> Option<PathBuf> {
    let req = req.try_clone()?.build().ok()?;
    let mut h = DefaultHasher::new();
    req.method().as_str().hash(&mut h);
    req.url().as_str().hash(&mut h);
    req.body().and_then(|b| b.as_bytes()).hash(&mut h);
    Some(dir.join(format!("{:016x}.json", h.finish())))
}

/// Run `fetch(req)`, or replay its recorded response when the cache is enabled.
pub async fn json<F, Fut>(req: reqwest::RequestBuilder, fetch: F) -> Result<Value>
where
    F: FnOnce(reqwest::RequestBuilder) -> Fut,
    Fut: Future<Output = Result<Value>>,
{
    if crate::mock::enabled() {
        let req = req.build()?;
        return crate::mock::response(req.method().as_str(), req.url().as_str())
            .ok_or_else(|| anyhow::anyhow!("LOOTINGGLASS_MOCK: no canned response for {}", req.url()));
    }
    let Some(path) = dir().and_then(|d| file(&d, &req)) else {
        return fetch(req).await;
    };
    if let Ok(s) = std::fs::read_to_string(&path) {
        log::debug!("http cache hit: {}", path.display());
        // Recording a demo: take as long as a real request, so "Searching…" shows up.
        if std::env::var_os("LOOTINGGLASS_RECORD").is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        }
        return Ok(serde_json::from_str(&s)?);
    }
    let v = fetch(req).await?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, v.to_string());
    Ok(v)
}
