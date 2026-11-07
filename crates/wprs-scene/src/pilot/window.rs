const SPAN: usize = 32;
const LEAST: usize = 12;

#[derive(Debug, Clone, Copy, Default)]
pub struct Window {
    times: [f32; SPAN],
    len: usize,
    next: usize,
}

impl Window {
    pub fn push(&mut self, ms: f32) {
        self.times[self.next] = ms;
        (self.next, self.len) = ((self.next + 1) % SPAN, (self.len + 1).min(SPAN));
    }

    pub fn mean(&self) -> Option<f32> {
        (self.len >= LEAST).then_some(())?;
        let mut sorted = self.times;
        let sorted = &mut sorted[..self.len];
        sorted.sort_unstable_by(f32::total_cmp);
        let median = sorted[self.len / 2];
        let kept = sorted.iter().filter(|t| **t <= median * 2.0 && **t >= median / 2.0);
        let (sum, n) = kept.fold((0.0, 0u32), |(s, n), t| (s + t, n + 1));
        Some(if n == 0 { median } else { sum / n as f32 })
    }
}
