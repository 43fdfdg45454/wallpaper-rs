use std::time::Duration;

pub use wprs_library::{Turn, rotate};

pub const CHUNK: Duration = Duration::from_secs(900);

pub fn nap(due_ms: Option<i64>, now_ms: i64) -> Option<Duration> {
    let max = CHUNK.as_millis() as i64;
    due_ms.map(|due| Duration::from_millis(due.saturating_sub(now_ms).clamp(0, max) as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn naps_last_at_most_one_chunk() {
        assert_eq!(nap(Some(10_000), 0), Some(Duration::from_secs(10)));
        assert_eq!(nap(Some(i64::MAX), 0), Some(CHUNK));
        assert_eq!(nap(Some(0), 5_000), Some(Duration::ZERO));
        assert_eq!(nap(None, 0), None);
    }
}
