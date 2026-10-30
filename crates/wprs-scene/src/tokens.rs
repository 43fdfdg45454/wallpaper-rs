pub(crate) fn word(b: &[u8], at: usize, name: &[u8]) -> bool {
    let after = b.get(at + name.len()).is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_');
    b[at..].starts_with(name) && !after
}

pub(crate) fn wide(b: &[u8], at: usize) -> usize {
    match b[at..] {
        [0x0b, ..] => 1,
        [0xc2, 0x85, ..] => 2,
        [0xe2, 0x80, 0x8e | 0x8f | 0xa8 | 0xa9, ..] => 3,
        _ => 0,
    }
}

pub(crate) fn line_end(b: &[u8], mut at: usize) -> usize {
    while at < b.len() {
        if matches!(
            b[at..],
            [b'\n' | 0x0b | 0x0c | b'\r', ..] | [0xc2, 0x85, ..] | [0xe2, 0x80, 0xa8 | 0xa9, ..]
        ) {
            return at;
        }
        at += 1;
    }
    at
}

pub(crate) fn comment(b: &[u8], mut at: usize, mut line: usize) -> (usize, usize) {
    let mut open = 1;
    while at < b.len() && open > 0 {
        match (b[at], b.get(at + 1)) {
            (b'/', Some(b'*')) => (open, at) = (open + 1, at + 1),
            (b'*', Some(b'/')) => (open, at) = (open - 1, at + 1),
            (b'\n', _) => line += 1,
            _ => {}
        }
        at += 1;
    }
    (at, line)
}

pub(crate) fn number(b: &[u8], start: usize) -> usize {
    let hex = b[start..].starts_with(b"0x") || b[start..].starts_with(b"0X");
    let digit = |c: &u8| if hex { c.is_ascii_hexdigit() } else { c.is_ascii_digit() };
    let (mut at, mut dot) = (start + if hex { 2 } else { 0 }, false);
    while let Some(c) = b.get(at) {
        let member = b.get(at + 1).is_some_and(|n| n.is_ascii_alphabetic() || *n == b'_');
        match c {
            c if digit(c) => {}
            b'.' if !dot && !member => dot = true,
            _ => break,
        }
        at += 1;
    }
    let exponent: &[u8] = if hex { b"pP" } else { b"eE" };
    if b.get(at).is_some_and(|c| exponent.contains(c)) {
        let sign = usize::from(matches!(b.get(at + 1), Some(b'+' | b'-')));
        let digits = b.iter().skip(at + 1 + sign).take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 {
            at += 1 + sign + digits;
        }
    }
    at + usize::from(b.get(at).is_some_and(|c| b"fhiu".contains(c)))
}
