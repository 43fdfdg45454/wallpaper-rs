use crate::{Error, info};

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

pub fn pairs(bytes: &[u8]) -> Vec<(&[u8], &[u8])> {
    let (Some(start), Some(length)) = (word(bytes, 56), word(bytes, 60)) else { return Vec::new() };
    let (mut at, end) = (start as usize, (start as usize).saturating_add(length as usize).min(bytes.len()));
    let mut found = Vec::new();
    while let Some(size) = word(bytes, at).filter(|_| at + 4 <= end) {
        let Some(entry) = bytes.get(at + 4..at + 4 + size as usize).filter(|_| at + 4 + size as usize <= end)
        else {
            break;
        };
        if let Some(nul) = entry.iter().position(|b| *b == 0) {
            found.push((&entry[..nul], &entry[nul + 1..]));
        }
        at += 4 + (size as usize).div_ceil(4) * 4;
    }
    found
}

pub fn durations(bytes: &[u8]) -> Result<Vec<f32>, Error> {
    let frames = info(bytes)?.frames as usize;
    let found = pairs(bytes);
    let own = found.iter().find(|(k, _)| *k == PACE).map(|(_, v)| *v).filter(|v| v.len() == frames * 4);
    if let Some(own) = own {
        return Ok((0..frames)
            .map(|i| (word(own, i * 4).unwrap_or(0) as f32 / 1000.0).clamp(0.02, 60.0))
            .collect());
    }
    let anim = found.iter().find(|(k, _)| *k == ANIMATION).map(|(_, v)| *v);
    let seconds = anim.and_then(|v| Some(word(v, 0)? as f32 / word(v, 4).filter(|t| *t > 0)? as f32));
    let seconds = seconds.filter(|s| s.is_finite() && *s >= 0.02).map_or(1.0 / 24.0, |s| s.min(60.0));
    Ok(vec![seconds; frames])
}

pub const QUALITY: &[u8] = b"wprsQuality";
pub const ANIMATION: &[u8] = b"KTXanimData";
pub const PACE: &[u8] = b"wprsFrameTimes";

pub fn quality(bytes: &[u8]) -> Option<u8> {
    let found = pairs(bytes).into_iter().find(|(k, _)| *k == QUALITY)?.1;
    found.first().copied().filter(|q| (1..=100).contains(q))
}

fn long(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn entries(list: &[(&[u8], &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (key, value) in list {
        out.extend(((key.len() + 1 + value.len()) as u32).to_le_bytes());
        out.extend(key.iter().chain(&[0]).chain(value.iter()));
        out.resize(out.len().div_ceil(4) * 4, 0);
    }
    out
}

pub fn with(bytes: &[u8], key: &[u8], value: &[u8]) -> Vec<u8> {
    placed(bytes, key, value).unwrap_or_else(|| bytes.to_vec())
}

fn placed(bytes: &[u8], key: &[u8], value: &[u8]) -> Option<Vec<u8>> {
    let levels = word(bytes, 40)?.max(1) as usize;
    let (dfd, kvd, kvd_len) =
        (word(bytes, 48)? as usize + word(bytes, 52)? as usize, word(bytes, 56)?, word(bytes, 60)?);
    let (sgd, sgd_len) = (long(bytes, 64)?, long(bytes, 72)?);
    let offsets: Vec<usize> = (0..levels).map(|l| 80 + l * 24).collect();
    let first = offsets.iter().map(|at| long(bytes, *at)).collect::<Option<Vec<_>>>()?.into_iter().min()?;
    let start = if kvd_len > 0 { kvd as usize } else { dfd };
    let next = if sgd_len > 0 { sgd } else { first } as usize;
    if start < 80 + levels * 24 || next < start + kvd_len as usize || next > bytes.len() {
        return None;
    }
    let mut list: Vec<_> = pairs(bytes).into_iter().filter(|(k, _)| *k != key).collect();
    list.push((key, value));
    list.sort_by(|a, b| a.0.cmp(b.0));
    let kv = entries(&list);
    let end = start + kv.len();
    let moved = next + (end.saturating_sub(next)).div_ceil(16) * 16;
    let moved = if moved >= end { moved } else { moved + 16 };
    let delta = moved as i64 - next as i64;
    let mut out = bytes[..start].to_vec();
    out.extend(&kv);
    out.resize(moved, 0);
    out.extend(&bytes[next..]);
    out[56..60].copy_from_slice(&(start as u32).to_le_bytes());
    out[60..64].copy_from_slice(&(kv.len() as u32).to_le_bytes());
    let shift = |out: &mut Vec<u8>, at: usize| {
        let old = long(out, at).unwrap_or(0);
        out[at..at + 8].copy_from_slice(&((old as i64 + delta) as u64).to_le_bytes());
    };
    if sgd_len > 0 {
        shift(&mut out, 64);
    }
    offsets.iter().for_each(|at| shift(&mut out, *at));
    Some(out)
}

fn ms(seconds: f32) -> u32 {
    (seconds.clamp(0.02, 60.0) * 1000.0).round() as u32
}

pub fn timed(bytes: &[u8], seconds: f32) -> Vec<u8> {
    let value: Vec<u8> = [ms(seconds), 1000, 0].iter().flat_map(|v| v.to_le_bytes()).collect();
    with(bytes, ANIMATION, &value)
}

pub fn paced(bytes: &[u8], seconds: &[f32]) -> Vec<u8> {
    let average = seconds.iter().sum::<f32>() / seconds.len().max(1) as f32;
    let even = timed(bytes, average);
    if seconds.windows(2).all(|w| ms(w[0]) == ms(w[1])) {
        return even;
    }
    let value: Vec<u8> = seconds.iter().flat_map(|s| ms(*s).to_le_bytes()).collect();
    with(&even, PACE, &value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glance {
    pub hdr: bool,
    pub frames: u32,
    pub cube: bool,
}

pub fn glance(head: &[u8]) -> Option<Glance> {
    let (frames, faces, dfd) = (word(head, 32)?.max(1), word(head, 36)?, word(head, 48)? as usize);
    let model = *head.get(dfd + 12)?;
    Some(Glance { hdr: matches!(model, 167 | 168), frames, cube: faces == 6 })
}
