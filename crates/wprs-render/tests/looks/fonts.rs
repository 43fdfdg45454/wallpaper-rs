use std::io::Write;

fn be<T: Copy, const N: usize>(values: &[T], f: impl Fn(T) -> [u8; N]) -> Vec<u8> {
    values.iter().flat_map(|v| f(*v)).collect()
}

pub(super) fn boxes() -> Vec<u8> {
    let (count, mut glyf, mut loca) = (97u16, Vec::new(), vec![0u32]);
    for g in 0..count {
        let h = match (g + 31) as u8 {
            _ if g == 96 => 100,
            b'A'..=b'Z' | b'0'..=b'9' => 700,
            b'a'..=b'z' => 500,
            b'!'..=b'~' => 200,
            _ => 0,
        };
        if h > 0 {
            glyf.extend(be(&[1i16, 50, 0, 550, h, 3, 0], i16::to_be_bytes));
            glyf.extend([1, 1, 1, 1]);
            glyf.extend(be(&[50i16, 500, 0, -500, 0, 0, h, 0, 0], i16::to_be_bytes));
        }
        loca.push(glyf.len() as u32);
    }
    let mut head =
        [&0x0001_0000u32.to_be_bytes()[..], &[0; 8], &0x5f0f_3cf5u32.to_be_bytes(), &[0; 2]].concat();
    head.extend(be(&[1000u16, 0, 0, 0, 0, 0, 0, 0, 0, 50, 0, 550, 700, 0, 8, 2, 1, 0], u16::to_be_bytes));
    let hhea = be(&[1u16, 0, 800, 0xff38, 0, 600, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, count], u16::to_be_bytes);
    let ranges = [0x7e, 0x2026, 0xffff, 0, 0x20, 0x2026, 0xffff, 0xffe1, 0xe03a, 1, 0, 0, 0];
    let cmap = be(&[&[0u16, 1, 3, 1, 0, 12, 4, 40, 0, 6, 4, 1, 2][..], &ranges].concat(), u16::to_be_bytes);
    let tables: [(&[u8; 4], Vec<u8>); 7] = [
        (b"cmap", cmap),
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", be(&[600u16, 50].repeat(count as usize), u16::to_be_bytes)),
        (b"loca", be(&loca, u32::to_be_bytes)),
        (b"maxp", [&0x5000u32.to_be_bytes()[..], &count.to_be_bytes()].concat()),
    ];
    let (mut out, mut body) = (be(&[1u16, 0, 7, 0, 0, 0], u16::to_be_bytes), Vec::new());
    for (tag, data) in &tables {
        let at = (12 + 16 * tables.len() + body.len()) as u32;
        out.extend([&tag[..], &[0; 4], &at.to_be_bytes(), &(data.len() as u32).to_be_bytes()].concat());
        body.extend(data);
        body.resize(body.len().next_multiple_of(4), 0);
    }
    out.extend(body);
    out
}

fn woff2(ttf: &[u8], tables: &[(&[u8], u32)], stream: &[u8], declared: u32) -> Vec<u8> {
    let mut packed = Vec::new();
    brotli::CompressorWriter::new(&mut packed, 4096, 5, 22).write_all(stream).unwrap();
    let mut directory = Vec::new();
    for &(tag, mut length) in tables {
        directory.extend([&[if matches!(tag, b"glyf" | b"loca") { 0xff } else { 0x3f }][..], tag].concat());
        let mut digits = vec![(length & 0x7f) as u8];
        while length > 0x7f {
            length >>= 7;
            digits.insert(0, (length & 0x7f) as u8 | 0x80);
        }
        directory.extend(digits);
    }
    let total = (48 + directory.len() + packed.len()) as u32;
    let mut out = [&b"wOF2"[..], &ttf[..4], &total.to_be_bytes()].concat();
    out.extend(be(&[tables.len() as u16, 0], u16::to_be_bytes));
    out.extend(be(&[declared, packed.len() as u32, 0x0001_0000, 0, 0, 0, 0, 0], u32::to_be_bytes));
    [out, directory, packed].concat()
}

pub(super) fn packed(ttf: &[u8], extra: Option<u32>, tail: usize) -> Vec<u8> {
    let tables = u16::from_be_bytes([ttf[4], ttf[5]]) as usize;
    let field = |o: usize| u32::from_be_bytes(ttf[o..o + 4].try_into().unwrap());
    let records: Vec<_> = (0..tables).map(|t| 12 + 16 * t).collect();
    let mut list: Vec<(&[u8], u32)> = records.iter().map(|&r| (&ttf[r..r + 4], field(r + 12))).collect();
    list.extend(extra.map(|length| (&b"zzzz"[..], length)));
    let mut stream: Vec<u8> = records
        .iter()
        .flat_map(|&r| ttf[field(r + 8) as usize..][..field(r + 12) as usize].to_vec())
        .collect();
    stream.resize(stream.len() + tail, 0);
    woff2(ttf, &list, &stream, ttf.len() as u32)
}
