use super::fonts::MAX_FONT;
use std::io::Read;

fn byte(b: &mut &[u8]) -> Option<u8> {
    let (&first, rest) = b.split_first()?;
    *b = rest;
    Some(first)
}

fn skip(b: &mut &[u8], n: usize) -> Option<()> {
    *b = b.get(n..)?;
    Some(())
}

fn base128(b: &mut &[u8]) -> Option<u64> {
    let mut value = 0u64;
    for _ in 0..5 {
        let next = byte(b)?;
        value = value << 7 | u64::from(next & 0x7f);
        if next & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn short(b: &mut &[u8]) -> Option<u16> {
    Some(match byte(b)? {
        253 => u16::from_be_bytes([byte(b)?, byte(b)?]),
        255 => u16::from(byte(b)?) + 253,
        254 => u16::from(byte(b)?) + 506,
        code => u16::from(code),
    })
}

pub(crate) fn fits(data: &[u8]) -> bool {
    unpacked(data).is_some_and(|(size, read, out)| size <= MAX_FONT as u64 && read <= out)
}

fn unpacked(data: &[u8]) -> Option<(u64, u64, u64)> {
    let tables = u16::from_be_bytes([*data.get(12)?, *data.get(13)?]);
    let mut b = data.get(48..)?;
    let (mut total, mut read) = (0u64, 0u64);
    for _ in 0..tables {
        let (flags, rest) = (byte(&mut b)?, b);
        let tag = (flags & 63 == 63).then(|| rest.get(..4)).flatten();
        skip(&mut b, if flags & 63 == 63 { 4 } else { 0 })?;
        let plain = base128(&mut b)?;
        let outlines = matches!(flags & 63, 10 | 11) || matches!(tag, Some(b"glyf" | b"loca"));
        let transformed = if outlines { flags >> 6 != 3 } else { flags >> 6 != 0 };
        let packed = if transformed { base128(&mut b)? } else { plain };
        (total, read) = (total + plain.max(packed), read + packed);
    }
    if data.get(4..8)? == b"ttcf" {
        skip(&mut b, 4)?;
        for _ in 0..short(&mut b)? {
            let count = short(&mut b)?;
            skip(&mut b, 4)?;
            (0..count).try_for_each(|_| short(&mut b).map(drop))?;
        }
    }
    let limit = MAX_FONT as u64 + 1;
    let mut stream = brotli_decompressor::Decompressor::new(b, 4096).take(limit);
    let out = std::io::copy(&mut stream, &mut std::io::sink()).ok()?;
    Some((total.max(out), read, out))
}
