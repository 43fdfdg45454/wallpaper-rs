use crate::Wake;
use crate::animate::Animate;
use crate::cover::FALLBACK;
use crate::input::MediaState;
use crate::media::{Media, Reading};
use crate::resolve::Context;

pub(crate) const BAR_FPS: u32 = 30;

pub(crate) fn clock(seconds: f32) -> String {
    let total = seconds.max(0.0).floor() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

pub(crate) fn position_at(state: &MediaState, spent: f64) -> f32 {
    if !state.present() {
        return 0.0;
    }
    let moved = match state.playing {
        true => state.position + spent.max(0.0) as f32 * state.rate,
        false => state.position,
    };
    if state.duration > 0.0 { moved.clamp(0.0, state.duration) } else { moved.max(0.0) }
}

impl Context<'_> {
    fn position(&self) -> f32 {
        position_at(self.media_state(), self.now - self.media.1)
    }

    fn ticking(&self, text: bool) {
        let s = self.media_state();
        let at = self.position();
        let ended = s.duration > 0.0 && ((s.rate > 0.0 && at >= s.duration) || (s.rate < 0.0 && at <= 0.0));
        if !s.present() || !s.playing || s.rate == 0.0 || ended {
            return;
        }
        match text {
            true => {
                let left = if s.rate > 0.0 { at.floor() + 1.0 - at } else { at - at.ceil() + 1.0 };
                self.note(Wake::At(f64::from(left.max(0.01) / s.rate.abs())));
            }
            false => self.paced(Wake::Frame, Some(BAR_FPS)),
        }
    }

    fn number(&self, m: Media, text: bool) -> (f32, String) {
        let s = self.media_state();
        let duration = if s.present() { s.duration } else { 0.0 };
        if m != Media::Duration {
            self.ticking(text);
        }
        let at = self.position();
        match m {
            Media::Position => (at, clock(at)),
            Media::Duration => (duration, clock(duration)),
            _ => {
                let p = if duration > 0.0 { (at / duration).clamp(0.0, 1.0) } else { 0.0 };
                (p, format!("{}%", (p * 100.0).floor()))
            }
        }
    }

    fn media_state(&self) -> &MediaState {
        &self.media.0
    }

    fn words(&self, m: Media) -> String {
        let s = self.media_state();
        if !s.present() {
            return String::new();
        }
        match m {
            Media::Title => s.title.clone(),
            Media::Artist => s.artist.clone(),
            Media::Album => s.album.clone(),
            Media::AlbumArtist => s.album_artist.clone(),
            Media::Subtitle => s.subtitle.clone(),
            Media::Genres => s.genres.clone(),
            _ => s.kind.clone(),
        }
    }

    pub(crate) fn media<T: Animate>(&self, m: Media) -> T {
        match m.reading() {
            Reading::Words => T::from_text(self.words(m)).unwrap_or_default(),
            Reading::Flag => {
                let s = self.media_state();
                T::from_signal(f32::from(u8::from(s.present() && s.playing))).unwrap_or_default()
            }
            Reading::Color => {
                let i = [Media::Primary, Media::Secondary, Media::Tertiary, Media::Text];
                let i = i.iter().position(|c| *c == m).unwrap_or(4);
                let colors = self.media_state().colors.unwrap_or(FALLBACK);
                T::from_json(&serde_json::json!(colors[i])).unwrap_or_default()
            }
            Reading::Number => {
                let text = T::from_signal(0.0).is_none();
                let (value, written) = self.number(m, text);
                T::from_signal(value).or_else(|| T::from_text(written)).unwrap_or_default()
            }
        }
    }
}
