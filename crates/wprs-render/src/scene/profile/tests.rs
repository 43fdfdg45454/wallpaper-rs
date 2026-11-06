use super::{SAMPLES, profiled_passes, sampled};
use std::sync::PoisonError;
use std::time::{Duration, Instant};

#[test]
fn a_pass_read_back_late_keeps_the_time_of_its_frame() {
    let Some(old) = Instant::now().checked_sub(Duration::from_secs(40)) else { return };
    let labels = vec!["stale pass".to_owned(), "other stale pass".to_owned()];
    let sample = sampled(old, labels, &[100, 1_100, 2_000, 2_500], 1_000.0);
    assert_eq!(sample.0, old);
    assert_eq!(sample.1, [("stale pass".to_owned(), 1.0), ("other stale pass".to_owned(), 0.5)]);
    let fresh = sampled(Instant::now(), vec!["fresh pass".to_owned()], &[0, 2_000], 1_000.0);
    {
        let mut samples = SAMPLES.lock().unwrap_or_else(PoisonError::into_inner);
        samples.push_back(sample);
        samples.push_back(fresh);
    }
    let (_, passes) = profiled_passes(Some(Duration::from_secs(30)));
    let named = |name: &str| passes.iter().any(|(label, _)| label == name);
    assert!(named("fresh pass") && !named("stale pass") && !named("other stale pass"), "{passes:?}");
}
