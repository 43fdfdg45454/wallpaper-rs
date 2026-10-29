use crate::x11::Rect;
use std::time::{Duration, Instant};

pub(crate) fn covers(area: Rect, rects: &[Rect]) -> bool {
    let clipped: Vec<Rect> = rects.iter().filter_map(|r| r.within(area)).collect();
    let mut xs: Vec<i32> =
        clipped.iter().flat_map(|r| [r.x, r.right()]).chain([area.x, area.right()]).collect();
    xs.sort_unstable();
    xs.dedup();
    xs.windows(2).all(|slab| {
        let mut spans: Vec<(i32, i32)> = clipped
            .iter()
            .filter(|r| r.x <= slab[0] && r.right() >= slab[1])
            .map(|r| (r.y, r.bottom()))
            .collect();
        spans.sort_unstable();
        let reach =
            spans.iter().try_fold(area.y, |reach, &(top, end)| (top <= reach).then_some(reach.max(end)));
        reach.is_some_and(|reach| reach >= area.bottom())
    })
}

pub(crate) fn rationed(history: &mut Vec<Instant>, now: Instant, most: usize, within: Duration) -> bool {
    history.retain(|at| now.saturating_duration_since(*at) < within);
    let allowed = history.len() < most;
    if allowed {
        history.push(now);
    }
    allowed
}

pub(crate) fn allowed_at(history: &[Instant], most: usize, within: Duration) -> Option<Instant> {
    history.len().checked_sub(most).map(|oldest| history[oldest] + within)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect { x, y, width, height }
    }

    #[test]
    fn windows_that_tile_a_screen_together_cover_it() {
        let screen = rect(0, 0, 1000, 500);
        assert!(covers(screen, &[rect(0, 0, 500, 500), rect(500, 0, 500, 500)]));
        assert!(covers(
            screen,
            &[rect(-10, -10, 600, 300), rect(400, -5, 700, 300), rect(0, 290, 1000, 300)]
        ));
        assert!(!covers(screen, &[rect(0, 0, 500, 500), rect(501, 0, 499, 500)]));
        assert!(!covers(screen, &[rect(0, 0, 1000, 499)]));
        assert!(!covers(screen, &[rect(1000, 0, 500, 500)]) && !covers(screen, &[]));
    }

    #[test]
    fn repeated_actions_are_rationed() {
        let (mut history, now, minute) = (Vec::new(), Instant::now(), Duration::from_secs(60));
        assert!((0..3).all(|i| rationed(&mut history, now + Duration::from_secs(i), 3, minute)));
        assert!(!rationed(&mut history, now + Duration::from_secs(10), 3, minute));
        assert!(rationed(&mut history, now + minute + Duration::from_secs(1), 3, minute));
    }

    #[test]
    fn a_rationed_action_knows_when_it_may_run_again() {
        let (mut history, now, minute) = (Vec::new(), Instant::now(), Duration::from_secs(60));
        assert_eq!(allowed_at(&history, 3, minute), None);
        assert!((0..3).all(|i| rationed(&mut history, now + Duration::from_secs(i), 3, minute)));
        let fourth = now + Duration::from_secs(5);
        assert!(!rationed(&mut history, fourth, 3, minute));
        let again = allowed_at(&history, 3, minute).expect("a time to try again");
        assert_eq!(again, now + minute);
        assert!(!rationed(&mut history, again - Duration::from_millis(1), 3, minute));
        assert!(rationed(&mut history, again, 3, minute));
    }
}
