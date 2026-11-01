const INFO: [u8; 4] = [0x15, 0x49, 0xa9, 0x66];
const SCALE: u64 = 0x2a_d7b1;
const DURATION: u64 = 0x4489;

fn find(b: &[u8], needle: &[u8]) -> Option<usize> {
    b.windows(needle.len()).position(|w| w == needle)
}

fn be(b: &[u8]) -> u64 {
    b.iter().fold(0, |v, x| v << 8 | u64::from(*x))
}

fn mvhd(b: &[u8]) -> Option<f32> {
    let at = find(b, b"mvhd")? + 4;
    let v1 = *b.get(at)? == 1;
    let (scale, duration) = match v1 {
        true => (be(b.get(at + 20..at + 24)?), be(b.get(at + 24..at + 32)?)),
        false => (be(b.get(at + 12..at + 16)?), be(b.get(at + 16..at + 20)?)),
    };
    (scale > 0).then(|| (duration as f64 / scale as f64) as f32)
}

fn vint(b: &[u8], at: usize, keep: bool) -> Option<(u64, usize)> {
    let first = *b.get(at)?;
    let n = first.leading_zeros() as usize + 1;
    if n > 8 {
        return None;
    }
    let head = if keep { u64::from(first) } else { u64::from(first) & (0xff >> n) };
    let rest = b.get(at + 1..at + n)?;
    Some((rest.iter().fold(head, |v, x| v << 8 | u64::from(*x)), n))
}

fn info(b: &[u8]) -> Option<f32> {
    let starts = b.windows(INFO.len()).enumerate().filter(|(_, w)| *w == INFO);
    starts.take(8).find_map(|(at, _)| element(b, at + INFO.len()))
}

fn element(b: &[u8], start: usize) -> Option<f32> {
    let (size, n) = vint(b, start, false)?;
    let (mut at, end) = (start + n, (start + n).saturating_add(size as usize).min(b.len()));
    let (mut scale, mut duration) = (1_000_000.0, None);
    while at < end {
        let (id, a) = vint(b, at, true)?;
        let (len, c) = vint(b, at + a, false)?;
        let data = b.get(at + a + c..at + a + c + len as usize)?;
        match (id, data.len()) {
            (SCALE, _) => scale = be(data) as f64,
            (DURATION, 4) => duration = Some(f64::from(f32::from_bits(be(data) as u32))),
            (DURATION, 8) => duration = Some(f64::from_bits(be(data))),
            _ => {}
        }
        at += a + c + len as usize;
    }
    duration.map(|d| (d * scale / 1e9) as f32)
}

pub fn film_seconds(head: &[u8], tail: &[u8]) -> Option<f32> {
    let found = info(head).or_else(|| mvhd(head)).or_else(|| mvhd(tail));
    found.filter(|s| s.is_finite() && *s > 0.0)
}

pub fn film_file(path: &std::path::Path) -> Option<f32> {
    use std::io::{Read, Seek, SeekFrom};
    const EDGE: u64 = 1 << 20;
    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let mut edge = |from: u64| {
        let mut bytes = Vec::new();
        file.seek(SeekFrom::Start(from)).and_then(|_| (&mut file).take(EDGE).read_to_end(&mut bytes)).ok()?;
        Some(bytes)
    };
    let (head, tail) = (edge(0)?, edge(size.saturating_sub(EDGE))?);
    film_seconds(&head, &tail)
}
