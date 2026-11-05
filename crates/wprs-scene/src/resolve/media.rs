use crate::animate::Animate;
use crate::cover::FALLBACK;
use crate::input::MediaState;
use crate::media::{Media, Reading};
use crate::resolve::Context;

impl Context<'_> {
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
            Reading::Color => {
                let i = [Media::Primary, Media::Secondary, Media::Tertiary, Media::Text];
                let i = i.iter().position(|c| *c == m).unwrap_or(4);
                let colors = self.media_state().colors.unwrap_or(FALLBACK);
                T::from_json(&serde_json::json!(colors[i])).unwrap_or_default()
            }
            _ => T::default(),
        }
    }
}
