const RATES: [[u32; 3]; 3] = [[44_100, 48_000, 32_000], [22_050, 24_000, 16_000], [11_025, 12_000, 8_000]];
const FIRST: [u32; 15] = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
const GAP: usize = 1024;
const SECOND: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];

fn be(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

pub(crate) fn mp3(b: &[u8], size: u64) -> Option<(u32, u32, u64)> {
    let mut start = 0usize;
    if b.starts_with(b"ID3") {
        let size = b.get(6..10)?.iter().fold(0usize, |v, &x| v << 7 | usize::from(x & 0x7f));
        start = 10 + size + if b.get(5)? & 0x10 != 0 { 10 } else { 0 };
    }
    let sync = |i: usize| b.get(i) == Some(&0xff) && b.get(i + 1).is_some_and(|x| x & 0xe6 == 0xe2);
    let at = (start..start.saturating_add(GAP)).find(|&i| sync(i))?;
    let head = be(b, at)?;
    let version = (head >> 19) & 3;
    let row = match version {
        3 => 0,
        2 => 1,
        0 => 2,
        _ => return None,
    };
    let (rate_index, bit_index) = (((head >> 10) & 3) as usize, ((head >> 12) & 15) as usize);
    let rate = *RATES[row].get(rate_index)?;
    let table = if row == 0 { &FIRST } else { &SECOND };
    let kbps = *table.get(bit_index).filter(|k| **k > 0)?;
    let mono = (head >> 6) & 3 == 3;
    let (samples, side) = match (row, mono) {
        (0, true) => (1152, 17),
        (0, false) => (1152, 32),
        (_, true) => (576, 9),
        (_, false) => (576, 17),
    };
    let padding = (head >> 9) & 1;
    let length = (samples / 8 * kbps * 1000 / rate + padding) as usize;
    (at + length + 4 > b.len() || sync(at + length)).then_some(())?;
    let tagged = |o: usize, tag: &[u8]| b.get(at + o..at + o + 4) == Some(tag);
    let frames = if tagged(4 + side, b"Xing") || tagged(4 + side, b"Info") {
        let base = at + 8 + side;
        (be(b, base)? & 1 == 1).then(|| be(b, base + 4)).flatten().map(u64::from)
    } else if tagged(36, b"VBRI") {
        be(b, at + 50).map(u64::from)
    } else {
        None
    };
    let frames = frames.unwrap_or(size.saturating_sub(at as u64) / length.max(1) as u64);
    (frames > 0).then_some((rate, if mono { 1 } else { 2 }, frames * u64::from(samples)))
}
