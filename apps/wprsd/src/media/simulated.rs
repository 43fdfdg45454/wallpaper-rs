use super::Frame;

fn song(f: &Frame) -> [&str; 4] {
    let s = &f.state;
    [&s.title, &s.artist, &s.album, &s.album_artist]
}

pub(crate) fn continued(before: Option<&Frame>, mut next: Frame) -> Frame {
    let Some(before) = before else { return next };
    let track = before.state.track.max(1);
    next.state.track = if song(before) == song(&next) { track } else { track + 1 };
    let digest = |f: &Frame| f.cover.as_ref().map(|c| c.digest);
    match digest(before) == digest(&next) {
        true => (next.cover, next.previous) = (before.cover.clone(), before.previous.clone()),
        false => next.previous = before.cover.clone().or_else(|| before.previous.clone()),
    }
    next.state.cover = next.cover.as_ref().map_or(0, |c| c.serial);
    next.state.colors = next.cover.as_ref().map(|c| c.colors);
    next
}

#[cfg(test)]
mod tests {
    use super::super::covers::decode;
    use super::*;
    use wprs_scene::MediaState;

    fn png(rgb: [u8; 3]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([rgb[0], rgb[1], rgb[2], 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn made(title: &str, cover: Option<[u8; 3]>, playing: bool) -> Frame {
        let cover = cover.and_then(|c| decode(&png(c)));
        let state =
            MediaState { enabled: true, track: 1, title: title.into(), playing, ..MediaState::default() };
        let state = MediaState { cover: cover.as_ref().map_or(0, |c| c.serial), ..state };
        Frame { cover, ..Frame::new(state) }
    }

    #[test]
    fn the_simulated_player_keeps_its_track_and_cover_until_they_change() {
        let first = made("One", Some([255, 0, 0]), true);
        let again = continued(Some(&first), made("One", Some([255, 0, 0]), false));
        assert_eq!((again.state.track, again.state.cover), (1, first.state.cover));
        assert!(again.previous.is_none() && !again.state.playing);
        let next = continued(Some(&again), made("Two", Some([0, 0, 255]), false));
        assert_eq!(next.state.track, 2);
        assert_ne!(next.state.cover, first.state.cover);
        assert_eq!(next.previous.as_ref().map(|c| c.serial), Some(first.state.cover));
        let bare = continued(Some(&next), made("Two", None, true));
        assert_eq!((bare.state.track, bare.state.cover, bare.state.colors), (2, 0, None));
        assert_eq!(bare.previous.as_ref().map(|c| c.serial), next.cover.as_ref().map(|c| c.serial));
    }
}
