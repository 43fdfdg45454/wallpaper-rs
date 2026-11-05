use super::View;
use std::time::Instant;

pub(super) fn allowed(view: &View) -> bool {
    view.pack.web.as_ref().is_some_and(|w| w.media) && crate::stage::media_allowed(&view.pack)
}

pub(super) fn told(view: &mut View, again: bool) -> Option<serde_json::Value> {
    let allowed = allowed(view) && crate::playing::access();
    let played = crate::playing::now().filter(|_| allowed);
    let cover = played.as_ref().and_then(|p| p.cover.clone());
    let now = (allowed, played.as_ref().map_or(0, |p| p.serial), cover.as_ref().map_or(0, |c| c.serial));
    let before = view.heard.replace(now).filter(|_| !again);
    if before.is_some_and(|(a, s, _)| (a, s) == (now.0, now.1)) {
        return None;
    }
    let fresh = before.is_none_or(|(_, _, c)| c != now.2);
    let state = played.map(|p| p.at(Instant::now()));
    let picture = fresh.then(|| cover.and_then(|c| c.pixels.image()).map(|i| png(&i)).unwrap_or_default());
    Some(wprs_format::page::heard(state.as_ref(), picture))
}

pub(crate) fn png(image: &image::RgbaImage) -> String {
    use base64::Engine;
    let mut out = std::io::Cursor::new(Vec::new());
    match image.write_to(&mut out, image::ImageFormat::Png) {
        Ok(()) => base64::engine::general_purpose::STANDARD.encode(out.into_inner()),
        Err(_) => String::new(),
    }
}
