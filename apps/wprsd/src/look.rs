use crate::compositor::{Content, Tone};
use std::path::Path;
use wprs_core::Fit;
use wprs_core::config::parse_hex_color;
use wprs_library::Kind;

pub type Look = (Option<Fit>, Option<[u8; 3]>);

pub(crate) fn display(path: &Path) -> wprs_format::Display {
    let package = Kind::detect(path) == Some(Kind::Package);
    let manifest = package.then(|| wprs_format::peek(path).ok()).flatten();
    manifest.and_then(|m| m.display).unwrap_or_default()
}

pub fn flip(display: &wprs_format::Display) -> [bool; 2] {
    let flip = display.flip.unwrap_or(wprs_format::Flip::None);
    [flip == wprs_format::Flip::Horizontal, flip == wprs_format::Flip::Vertical]
}

pub fn tone(display: &wprs_format::Display, shown: &Content, filter: wprs_core::Filter) -> Tone {
    let shows = matches!(shown, Content::Image(_) | Content::Video(_));
    Tone { flip: if shows { flip(display) } else { [false; 2] }, grade: filter.grade() }
}

pub fn author(display: &wprs_format::Display) -> Look {
    let span = display.span == Some(wprs_format::Span::AllOutputs);
    (display.fit.or(span.then_some(Fit::Span)), display.background.as_deref().and_then(parse_hex_color))
}

pub fn look((fit, background): Look, author: impl FnOnce() -> Look) -> (Fit, [u8; 3]) {
    let (by_fit, by_background) = if fit.is_some() && background.is_some() { (None, None) } else { author() };
    (fit.or(by_fit).unwrap_or_default(), background.or(by_background).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_author_look_fills_only_what_the_user_left_open() {
        let author = || (Some(Fit::Span), Some([255, 0, 0]));
        assert_eq!(look((None, None), author), (Fit::Span, [255, 0, 0]));
        assert_eq!(look((None, Some([1, 2, 3])), author), (Fit::Span, [1, 2, 3]));
        assert_eq!(look((Some(Fit::Center), None), author), (Fit::Center, [255, 0, 0]));
        assert_eq!(look((Some(Fit::Center), Some([1, 2, 3])), || unreachable!()), (Fit::Center, [1, 2, 3]));
        assert_eq!(look((None, None), || (None, None)), (Fit::default(), [0, 0, 0]));
    }

    #[test]
    fn the_author_look_reads_only_the_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::create_dir(&source).unwrap();
        let png = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 255, 255]));
        ["image.png", "preview.png"].iter().for_each(|name| png.save(source.join(name)).unwrap());
        let manifest = serde_json::json!({"schema": wprs_format::SCHEMA, "id": "org.example.look", "version": "1.0.0",
            "title": "Look", "license": "CC0-1.0", "kind": "image", "entry": "image.png", "content": {"rating": "general"},
            "photosensitive": false, "origin": {"kind": "original"}, "previews": {"thumbnail": "preview.png"},
            "requires": {"features": ["flip"]}, "display": {"flip": "horizontal", "background": "#ff0000"}});
        std::fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
        let wpak = dir.path().join("look.wpak");
        wprs_format::build(&source, &wpak, &wprs_format::Limits::default()).unwrap();
        assert!(crate::decode::open(&wpak).is_ok());
        assert_eq!(author(&display(&wpak)), (None, Some([255, 0, 0])));
        assert_eq!(flip(&display(&wpak)), [true, false]);
    }
}
