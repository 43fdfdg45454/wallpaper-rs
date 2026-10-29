use crate::validate::extension;
use crate::{Facts, Kind, Manifest, Result, Scene, require};

pub(crate) const EDGE: u64 = 4 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Media {
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
}

pub fn probe(data: &[u8]) -> Option<Media> {
    let at = |i: usize, n: usize| data.get(i..i + n);
    let le = |i: usize, n: usize| at(i, n).map(|b| b.iter().rev().fold(0u32, |a, x| a << 8 | u32::from(*x)));
    let be = |i: usize, n: usize| at(i, n).map(|b| b.iter().fold(0u32, |a, x| a << 8 | u32::from(*x)));
    let media = |format, width, height, duration_ms| Some(Media { format, width, height, duration_ms });
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return media("png", be(16, 4)?, be(20, 4)?, 0);
    }
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return media("gif", le(6, 2)?, le(8, 2)?, gif(data)?);
    }
    if data.starts_with(b"\xff\xd8") {
        return jpeg(data).and_then(|(w, h)| media("jpeg", w, h, 0));
    }
    if at(0, 4)? != b"RIFF" || at(8, 4)? != b"WEBP" {
        return None;
    }
    let (width, height) = match at(12, 4)? {
        b"VP8 " => (le(26, 2)? & 0x3fff, le(28, 2)? & 0x3fff),
        b"VP8L" => (1 + (le(21, 4)? & 0x3fff), 1 + (le(21, 4)? >> 14 & 0x3fff)),
        b"VP8X" => (1 + le(24, 3)?, 1 + le(27, 3)?),
        _ => return None,
    };
    let (mut at_chunk, mut duration) = (12usize, 0u64);
    while let (Some(name), Some(size)) = (at(at_chunk, 4), le(at_chunk + 4, 4)) {
        if name == b"ANMF" {
            duration += u64::from(le(at_chunk + 20, 3)?);
        }
        at_chunk += 8 + size as usize + (size as usize & 1);
    }
    media("webp", width, height, duration)
}

fn gif(data: &[u8]) -> Option<u64> {
    let table = |flags: u8| if flags & 0x80 != 0 { 3 << ((flags & 7) + 1) } else { 0 };
    let (mut i, mut centis) = (13 + table(*data.get(10)?), 0u64);
    let skip = |mut i: usize| -> Option<usize> {
        while *data.get(i)? != 0 {
            i += usize::from(data[i]) + 1;
        }
        Some(i + 1)
    };
    loop {
        match *data.get(i)? {
            0x21 if data.get(i + 1) == Some(&0xf9) => {
                let delay = u16::from_le_bytes([*data.get(i + 4)?, *data.get(i + 5)?]);
                centis += if delay < 2 { 10 } else { u64::from(delay) };
                i = skip(i + 2)?;
            }
            0x21 => i = skip(i + 2)?,
            0x2c => i = skip(i + 11 + table(*data.get(i + 9)?))?,
            0x3b => return Some(centis * 10),
            _ => return None,
        }
    }
}

fn jpeg(data: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    loop {
        let (marker, len) =
            (*data.get(i + 1)?, usize::from(u16::from_be_bytes([*data.get(i + 2)?, *data.get(i + 3)?])));
        if data[i] != 0xff {
            return None;
        }
        if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            let size =
                |o: usize| Some(u32::from(u16::from_be_bytes([*data.get(i + o)?, *data.get(i + o + 1)?])));
            return Some((size(7)?, size(5)?));
        }
        i += 2 + len;
    }
}

pub fn shown(extension: &str) -> bool {
    ["png", "jpg", "jpeg", "webp", "gif", "bmp", "webm", "mp4", "mkv"].contains(&extension)
}

pub fn preview(animated: bool, path: &str, data: &[u8]) -> Result<()> {
    let (field, side, seconds) =
        if animated { ("previews.animated", 640, 15) } else { ("previews.thumbnail", 1024, 0) };
    let ext = extension(path).unwrap_or_default().replace("jpg", "jpeg");
    let long = |m: &Media| seconds == 0 || m.duration_ms <= seconds * 1000;
    let fits = probe(data).is_some_and(|m| m.format == ext && m.width.max(m.height) <= side && long(&m));
    let most = if seconds > 0 { format!(" and {seconds} s") } else { String::new() };
    require(fits, field, &format!("{path:?} must be a {ext} image of at most {side} pixels a side{most}"))
}

pub fn hdr(data: &[u8]) -> bool {
    let tone = |t: u16| t == 16 || t == 18;
    let boxed = data.windows(12).any(|w| {
        &w[..4] == b"colr"
            && matches!(&w[4..8], b"nclx" | b"nclc")
            && tone(u16::from_be_bytes([w[10], w[11]]))
    });
    let tagged = data.windows(4).any(|w| w[..3] == [0x55, 0xba, 0x81] && tone(u16::from(w[3])));
    boxed || data.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) && tagged
}

pub(crate) fn videos(manifest: &Manifest, scene: Option<&Scene>) -> Vec<String> {
    let video = |p: &String| extension(p).is_some_and(|e| ["webm", "mp4", "mkv"].contains(&e.as_str()));
    let entry = (manifest.kind == Kind::Video).then(|| manifest.entry.clone());
    let assets = scene.into_iter().flat_map(|s| s.assets.values().map(|a| a.path.clone()));
    let mut found: Vec<String> = entry.into_iter().chain(assets).filter(video).collect();
    found.sort();
    found.dedup();
    found.truncate(16);
    found
}

pub(crate) fn mark(facts: &mut Facts, path: &str) {
    facts.facets.extend((!facts.facets.iter().any(|f| f == "hdr")).then(|| "hdr".to_owned()));
    facts.facets.sort();
    facts
        .notes
        .push(format!("{path} is an HDR video, which plays with standard colors and may look washed out"));
}
