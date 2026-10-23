pub(crate) fn within(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= max
}

pub(crate) fn safe(text: &str) -> bool {
    !text.contains(['<', '>'])
}

pub(crate) fn clean(text: &str, max: usize, lines: bool) -> bool {
    let bad = |c: char| c == '\u{feff}' || (c.is_control() && !(lines && c == '\n'));
    text.chars().count() <= max && !text.chars().any(bad)
}

pub(crate) fn valid_property(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_alphabetic())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(crate) fn valid_name(name: &str) -> bool {
    name.len() <= 63
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

pub(crate) fn valid_id(id: &str) -> bool {
    id.len() <= 255 && id.split('.').count() >= 2 && id.split('.').all(valid_name)
}

#[derive(Clone, Copy, PartialEq)]
enum Next {
    Operand,
    Exception,
    Operator { with: bool },
}

pub(crate) fn valid_license(expr: &str) -> bool {
    let spaced = expr.replace('(', " ( ").replace(')', " ) ");
    let end = spaced.split_whitespace().try_fold((Next::Operand, 0usize), |(next, depth), token| {
        match (token, next) {
            ("(", Next::Operand) => Some((Next::Operand, depth + 1)),
            (")", Next::Operator { .. }) if depth > 0 => Some((Next::Operator { with: false }, depth - 1)),
            ("AND" | "OR", Next::Operator { .. }) => Some((Next::Operand, depth)),
            ("WITH", Next::Operator { with: true }) => Some((Next::Exception, depth)),
            ("(" | ")" | "AND" | "OR" | "WITH", _) | (_, Next::Operator { .. }) => None,
            (id, _) => {
                let id = id.strip_suffix('+').unwrap_or(id);
                let valid = id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':'));
                (valid && !id.is_empty()).then_some((Next::Operator { with: next == Next::Operand }, depth))
            }
        }
    });
    matches!(end, Some((Next::Operator { .. }, 0)))
}
