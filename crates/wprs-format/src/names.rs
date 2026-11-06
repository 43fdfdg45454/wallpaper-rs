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

pub fn valid_id(id: &str) -> bool {
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

pub const MAX_VOTED: usize = 100;
pub const MAX_ASKED: usize = 1000;

pub fn vote_queries(asked: &[(&str, &str)]) -> Vec<String> {
    let mut ids: Vec<&str> = asked.iter().map(|(_, id)| *id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.chunks(MAX_VOTED).map(|chunk| format!("/api/v1/votes?ids={}", chunk.join(","))).collect()
}

pub fn voted<'a>(specs: &'a [String], server: Option<&str>) -> Option<Vec<(&'a str, &'a str)>> {
    let split = |spec: &'a str| match spec.split_once('/') {
        Some((repo, id)) => (Some(repo), id),
        None => (None, spec),
    };
    let parts: Vec<_> = specs.iter().map(|s| (s.as_str(), split(s))).collect();
    let fine = |(repo, id): (Option<&str>, &str)| valid_id(id) && repo.is_none_or(|r| within(r, 128));
    if specs.len() > MAX_ASKED || !parts.iter().all(|(_, p)| fine(*p)) {
        return None;
    }
    let ours = |repo: Option<&str>| repo.is_none_or(|r| server.is_some_and(|s| !s.is_empty() && s == r));
    Some(parts.into_iter().filter(|(_, (repo, _))| ours(*repo)).map(|(spec, (_, id))| (spec, id)).collect())
}
