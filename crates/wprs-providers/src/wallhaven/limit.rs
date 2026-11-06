use super::Wallhaven;
use crate::Result;
use reqwest::{Response, StatusCode, header::RETRY_AFTER};
use serde::de::DeserializeOwned;
use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};

const PER_WINDOW: usize = 45;
const RETRIES: u32 = 3;
const LONGEST: Duration = Duration::from_secs(30);

static SENT: LazyLock<Mutex<HashMap<String, VecDeque<Instant>>>> = LazyLock::new(Mutex::default);

async fn pace(base: &str, window: Duration) {
    loop {
        let wait = {
            let mut all = SENT.lock().unwrap_or_else(PoisonError::into_inner);
            let sent = all.entry(base.to_owned()).or_default();
            let now = Instant::now();
            while sent.front().is_some_and(|at| now.duration_since(*at) >= window) {
                sent.pop_front();
            }
            match sent.front() {
                Some(first) if sent.len() >= PER_WINDOW => window.saturating_sub(now.duration_since(*first)),
                _ => {
                    sent.push_back(now);
                    return;
                }
            }
        };
        tokio::time::sleep(wait).await;
    }
}

fn told(response: &Response) -> Option<Duration> {
    let seconds = response.headers().get(RETRY_AFTER)?.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(seconds))
}

impl Wallhaven {
    pub(super) async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, String)]) -> Result<T> {
        let mut delay = self.backoff;
        let mut attempt = 0;
        loop {
            pace(&self.base, self.window).await;
            let request = self.client.get(format!("{}{path}", self.base)).query(query);
            let request = self.api_key.iter().fold(request, |request, key| request.header("X-API-Key", key));
            let response = request.send().await?;
            if response.status() == StatusCode::TOO_MANY_REQUESTS && attempt < RETRIES {
                tokio::time::sleep(told(&response).unwrap_or(delay).min(LONGEST)).await;
                (attempt, delay) = (attempt + 1, delay * 2);
                continue;
            }
            return Ok(response.error_for_status()?.json().await?);
        }
    }
}
