mod analysis;
mod capture;

pub use analysis::{Analyzer, WINDOW, db01, smooth};
pub use capture::{Capture, Feed};
pub use wprs_scene::AudioFrame;

use std::time::Instant;

pub struct Audio {
    analyzer: Analyzer,
    feed: Option<Feed>,
    last: Option<Instant>,
}

impl Default for Audio {
    fn default() -> Self {
        Audio { analyzer: Analyzer::new(48_000), feed: None, last: None }
    }
}

impl Audio {
    pub fn attach(&mut self, feed: Option<Feed>) {
        *self = Audio { feed, ..Audio::default() };
    }

    pub fn attached(&self) -> bool {
        self.feed.is_some()
    }

    pub fn frame(&mut self) -> AudioFrame {
        let Some(feed) = &mut self.feed else { return AudioFrame::default() };
        self.analyzer.set_rate(feed.rate());
        feed.drain(|samples| self.analyzer.push(samples));
        let now = Instant::now();
        let dt = self.last.replace(now).map_or(0.0, |last| (now - last).as_secs_f32());
        self.analyzer.update(dt).clone()
    }
}
