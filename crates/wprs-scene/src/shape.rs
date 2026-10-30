use crate::tokens::{comment, line_end, number, wide, word};

pub const MAX_OPERANDS: usize = 256;
pub const MAX_BRACKETS: usize = 64;
pub const MAX_ARGUMENTS: usize = 4096;

const OPERATORS: &[u8] = b"+-*/%&|^!~<>=.";

#[derive(Default)]
struct Group {
    deepest: usize,
    ops: usize,
    inner: usize,
    called: bool,
}

fn group(groups: &mut [Group]) -> &mut Group {
    groups.last_mut().expect("the outer group stays")
}

impl Group {
    fn end(&mut self) {
        self.deepest = self.deepest.max(self.ops + self.inner);
        (self.ops, self.inner) = (0, 0);
    }
}

pub(crate) fn shape(source: &str) -> Result<(), String> {
    let b = source.as_bytes();
    let (mut at, mut line, mut ended, mut closed, mut arguments) = (0, 1, false, false, 0);
    let mut groups = vec![Group::default()];
    while at < b.len() {
        if wide(b, at) > 0 {
            at += wide(b, at);
            continue;
        }
        let (c, next, start) = (b[at], b.get(at + 1).copied().unwrap_or(0), at);
        let blank = c.is_ascii_whitespace() || (c == b'/' && matches!(next, b'/' | b'*'));
        if !blank && std::mem::take(&mut ended) && !word(b, at, b"else") {
            group(&mut groups).end();
        }
        if !blank && std::mem::take(&mut closed) && c == b'[' {
            group(&mut groups).ops += 1;
        }
        at += 1;
        match c {
            b'\n' => line += 1,
            b'/' if next == b'/' => at = line_end(b, at),
            b'/' if next == b'*' => (at, line) = comment(b, at + 1, line),
            b'0'..=b'9' => at = number(b, start),
            b'.' if next.is_ascii_digit() => at = number(b, start),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                group(&mut groups).ops += usize::from(word(b, start, b"else"));
                at = start
                    + b[start..].iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_').count();
            }
            b'(' | b'[' | b'{' => {
                arguments += usize::from(c == b'(');
                groups.push(Group { called: c == b'(', ..Group::default() });
                if groups.len() > MAX_BRACKETS {
                    return Err(format!("its shader nests more than {MAX_BRACKETS} brackets on line {line}"));
                }
            }
            b')' | b']' | b'}' if groups.len() > 1 => {
                let mut shut = groups.pop().unwrap_or_default();
                shut.end();
                let parent = group(&mut groups);
                parent.inner = parent.inner.max(shut.deepest + 1);
                (ended, closed) = (c == b'}', c != b'}');
            }
            b',' | b';' => {
                arguments += usize::from(c == b',' && group(&mut groups).called);
                group(&mut groups).end();
            }
            c if OPERATORS.contains(&c) => group(&mut groups).ops += 1,
            _ => {}
        }
        if groups.last().is_some_and(|g| g.ops + g.inner > MAX_OPERANDS) {
            return Err(format!(
                "its shader has an expression more than {MAX_OPERANDS} operations deep on line {line}"
            ));
        }
        if arguments > MAX_ARGUMENTS {
            return Err(format!(
                "its shader passes more than {MAX_ARGUMENTS} arguments to calls and constructors"
            ));
        }
    }
    Ok(())
}
