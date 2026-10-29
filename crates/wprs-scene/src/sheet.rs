use crate::look::Playback;
use crate::resolve::Context;
use crate::{MAX_FRAMES, Quality, Wake};

pub(crate) fn frame(cx: &Context, id: &str, playback: Option<&Playback>, quality: Quality) -> Option<u32> {
    let asset = cx.scene.assets.get(id).filter(|a| a.moves())?;
    let decoded = cx.sheets.get(id).filter(|_| asset.animated);
    let count = decoded.map_or(asset.cells(), |d| d.len() as u32).clamp(1, MAX_FRAMES);
    let count = asset.frames.unwrap_or(count).clamp(1, count);
    let last = count - 1;
    if let Some(picked) = playback.and_then(|p| p.frame.as_ref()) {
        return Some((picked.get(cx).max(0.0) as u32).min(last));
    }
    let (list, fixed, floor): (&[f32], _, _) = match (&asset.durations, asset.fps, decoded) {
        (d, _, _) if !d.is_empty() => (d, None, 0.0),
        (_, Some(fps), _) => (&[], Some(1.0 / fps.max(0.1)), 0.0),
        (_, None, Some(d)) => (d, None, 0.01),
        _ => return Some(0),
    };
    let n = if fixed.is_some() { count as usize } else { list.len().min(count as usize) };
    let durations = (0..n).map(|i| fixed.unwrap_or_else(|| list[i].max(floor)));
    let total: f32 = durations.clone().sum();
    let still = Playback::default();
    let p = playback.unwrap_or(&still);
    let start = p.start.clamp(0.0, total);
    let end = p.end.unwrap_or(total).clamp(start, total);
    let rate = p.rate.get(cx).clamp(0.0, 16.0);
    let span = end - start;
    let runs = quality > Quality::Q0 && p.playing.get(cx) && rate > 0.0 && span > 0.0;
    let ran = cx.t * rate;
    let at = match (runs, p.repeat) {
        (false, _) => start,
        (true, true) => start + ran.rem_euclid(span),
        (true, false) => (start + ran).min(end),
    };
    let (mut index, mut edge) = (0, 0.0);
    for (i, d) in durations.enumerate() {
        (index, edge) = (i, edge + d);
        if at < edge {
            break;
        }
    }
    if runs && (p.repeat || at < end) {
        let next = if p.repeat && edge >= end { end } else { edge.min(end) };
        cx.note(Wake::At(f64::from(((next - at) / rate).max(1e-3))));
    }
    Some((index as u32).min(last))
}
