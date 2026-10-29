pub const MAX_SOUND_BYTES: u64 = 8 << 20;
pub const MAX_SOUND_SECONDS: f32 = 60.0;
pub const MAX_SOUND_TOTAL: f32 = 180.0;
pub const MAX_STREAM_BYTES: u64 = 32 << 20;
pub const MAX_STREAM_SECONDS: f32 = 600.0;
pub const MAX_STREAM_TOTAL: u64 = 64 << 20;
pub const MAX_CLIPS: usize = 16;
const FORMATS: [&str; 5] = ["ogg", "opus", "flac", "wav", "mp3"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clip {
    pub rate: u32,
    pub channels: u32,
    pub seconds: f32,
}

pub fn playable(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, ext)| FORMATS.contains(&ext.to_ascii_lowercase().as_str()))
}

impl Clip {
    pub fn probe(name: &str, bytes: &[u8]) -> Result<Clip, String> {
        let clip = Clip::read(name, bytes)?;
        match clip.seconds > MAX_SOUND_SECONDS {
            true => Err(format!("lasts {:.1} s, at most {MAX_SOUND_SECONDS} s are allowed", clip.seconds)),
            false => Ok(clip),
        }
    }

    pub fn read(name: &str, bytes: &[u8]) -> Result<Clip, String> {
        Clip::sample(name, bytes, bytes, bytes.len() as u64)
    }

    pub fn sample(name: &str, head: &[u8], tail: &[u8], size: u64) -> Result<Clip, String> {
        let mp3 = name.rsplit_once('.').is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("mp3"));
        let (rate, channels, frames) = wav(head, size)
            .or_else(|| flac(head))
            .or_else(|| ogg(head, tail))
            .or_else(|| mp3.then(|| crate::mpeg::mp3(head, size)).flatten())
            .ok_or("is not an Ogg Vorbis, Opus, FLAC or WAV sound, or an MP3 sound named .mp3")?;
        if !(1..=2).contains(&channels) {
            return Err(format!("has {channels} channels, expected mono or stereo"));
        }
        if !(8_000..=192_000).contains(&rate) {
            return Err(format!("is sampled at {rate} Hz, expected 8000 to 192000 Hz"));
        }
        let seconds = frames as f32 / rate as f32;
        if seconds > MAX_STREAM_SECONDS {
            return Err(format!("lasts {seconds:.1} s, at most {MAX_STREAM_SECONDS} s are allowed"));
        }
        Ok(Clip { rate, channels, seconds })
    }
}

fn le(bytes: &[u8], at: usize, len: usize) -> Option<u64> {
    let field = bytes.get(at..at.checked_add(len)?)?;
    Some(field.iter().rev().fold(0, |v, &b| v << 8 | u64::from(b)))
}

fn wav(b: &[u8], total: u64) -> Option<(u32, u32, u64)> {
    (b.get(..4)? == b"RIFF" && b.get(8..12)? == b"WAVE").then_some(())?;
    let (mut at, mut format, mut data) = (12usize, None, None);
    while let (Some(id), Some(size)) = (b.get(at..at + 4), le(b, at + 4, 4)) {
        let body = at + 8;
        match id {
            b"fmt " => format = Some((le(b, body + 4, 4)?, le(b, body + 2, 2)?, le(b, body + 8, 4)?)),
            b"data" => data = Some(size.min(total.saturating_sub(body as u64))),
            _ => {}
        }
        at = body.checked_add(usize::try_from(size + (size & 1)).ok()?)?;
    }
    let ((rate, channels, per_second), size) = (format?, data?);
    (per_second > 0).then(|| (rate as u32, channels as u32, size * rate / per_second))
}

fn flac(b: &[u8]) -> Option<(u32, u32, u64)> {
    (b.get(..4)? == b"fLaC" && b.get(4)? & 0x7f == 0).then_some(())?;
    let info = u64::from_be_bytes(b.get(18..26)?.try_into().ok()?);
    let frames = info & 0xf_ffff_ffff;
    (frames > 0).then_some(((info >> 44) as u32, ((info >> 41) & 7) as u32 + 1, frames))
}

fn ogg(b: &[u8], tail: &[u8]) -> Option<(u32, u32, u64)> {
    (b.get(..4)? == b"OggS").then_some(())?;
    let packet = b.get(27 + usize::from(*b.get(26)?)..)?;
    let (rate, channels, skip) = if packet.starts_with(b"\x01vorbis") {
        (le(packet, 12, 4)?, *packet.get(11)?, 0)
    } else if packet.starts_with(b"OpusHead") {
        (48_000, *packet.get(9)?, le(packet, 10, 2)?)
    } else {
        return None;
    };
    let serial = le(b, 14, 4)?;
    let page = |at: usize| {
        let page = &tail[at..];
        (page.starts_with(b"OggS") && le(page, 14, 4) == Some(serial)).then_some(())?;
        let segments = 27 + usize::from(*page.get(26)?);
        let body: usize = page.get(27..segments)?.iter().map(|&s| usize::from(s)).sum();
        (u64::from(crc(page.get(..segments + body)?)) == le(page, 22, 4)?).then_some(())?;
        Some((le(page, 6, 8)?, page[5] & 4 != 0)).filter(|(granule, _)| *granule != u64::MAX)
    };
    let mut pages = (0..=tail.len().saturating_sub(27)).rev().filter_map(page);
    let (last, _) = pages.clone().find(|(_, end)| *end).or_else(|| pages.next())?;
    Some((rate as u32, u32::from(channels), last.saturating_sub(skip)))
}

fn crc(page: &[u8]) -> u32 {
    let bytes = page.iter().enumerate().map(|(i, &b)| if (22..26).contains(&i) { 0 } else { b });
    bytes.fold(0, |crc, b| {
        (0..8).fold(crc ^ u32::from(b) << 24, |c, _| if c >> 31 == 1 { c << 1 ^ 0x04c1_1db7 } else { c << 1 })
    })
}
