enum Piece<'a> {
    Text(&'a str),
    Bold,
    Italic,
    Link(&'a str, &'a str),
    Star(&'a str, bool),
}

pub fn check(text: &str) -> Result<(), String> {
    let html = text.char_indices().any(|(i, c)| {
        c == '<' && text[i + 1..].starts_with(|n: char| n.is_ascii_alphabetic() || "/!?".contains(n))
    });
    if html || text.contains("![") {
        return Err(
            "HTML and images are not allowed; use paragraphs, lists, **bold**, *italic* and links".into()
        );
    }
    let pieces = text.lines().flat_map(|l| pieces(item(l.trim()).unwrap_or(l))).collect::<Vec<_>>();
    let bad =
        |p: &&Piece| matches!(p, Piece::Link(_, url) if !https(url)) || matches!(p, Piece::Star(_, true));
    match pieces.iter().find(bad) {
        Some(Piece::Link(_, url)) => Err(format!("link {url:?} must be an https:// address")),
        Some(_) => Err("an * or ** has no partner on its line; write \\* for an asterisk".into()),
        None => Ok(()),
    }
}

pub fn verbatim(text: &str) -> String {
    let escaped = escape(text.trim());
    format!("<p>{}</p>", escaped.replace("\n\n", "</p><p>").replace('\n', "<br>"))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn html(text: &str) -> String {
    let line = |l: &str| {
        let (mut out, mut bold, mut italic) = (String::new(), false, false);
        for piece in pieces(l) {
            out += &match piece {
                Piece::Text(t) | Piece::Star(t, _) => escape(t),
                Piece::Bold => toggle(&mut bold, "b"),
                Piece::Italic => toggle(&mut italic, "i"),
                Piece::Link(label, url) if https(url) => {
                    format!("<a href=\"{}\">{}</a>", escape(url), escape(label))
                }
                Piece::Link(label, url) => escape(&format!("[{label}]({url})")),
            };
        }
        out + if italic { "</i>" } else { "" } + if bold { "</b>" } else { "" }
    };
    blocks(text, |items, list| match list {
        true => {
            format!("<ul>{}</ul>", items.iter().map(|i| format!("<li>{}</li>", line(i))).collect::<String>())
        }
        false => format!("<p>{}</p>", items.iter().map(|i| line(i)).collect::<Vec<_>>().join("<br>")),
    })
}

pub fn plain(text: &str) -> String {
    let line = |l: &str| {
        pieces(l)
            .into_iter()
            .map(|p| match p {
                Piece::Text(t) | Piece::Star(t, _) => t.to_owned(),
                Piece::Bold | Piece::Italic => String::new(),
                Piece::Link(label, url) if label == url => url.to_owned(),
                Piece::Link(label, url) => format!("{label} ({url})"),
            })
            .collect::<String>()
    };
    let block = |items: &[&str], list: bool| {
        items
            .iter()
            .map(|i| if list { format!("• {}", line(i)) } else { line(i) })
            .collect::<Vec<_>>()
            .join("\n")
    };
    blocks(text, block).trim_end().to_owned()
}

fn item(line: &str) -> Option<&str> {
    line.strip_prefix("- ").or_else(|| line.strip_prefix("* "))
}

fn toggle(on: &mut bool, tag: &str) -> String {
    *on = !*on;
    if *on { format!("<{tag}>") } else { format!("</{tag}>") }
}

fn blocks(text: &str, render: impl Fn(&[&str], bool) -> String) -> String {
    let mut out = String::new();
    for block in text.split("\n\n").map(str::trim).filter(|b| !b.is_empty()) {
        let lines: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let separator = if out.is_empty() { "" } else { "\n\n" };
        out += separator;
        out += &match lines.iter().all(|l| item(l).is_some()) {
            true => render(&lines.iter().filter_map(|l| item(l)).collect::<Vec<_>>(), true),
            false => render(&lines, false),
        };
    }
    out
}

fn pieces(line: &str) -> Vec<Piece<'_>> {
    let (mut out, mut at) = (Vec::new(), 0);
    let solid = |c: Option<char>| c.is_some_and(|c| !c.is_whitespace());
    while at < line.len() {
        let rest = &line[at..];
        let link = rest.strip_prefix('[').and_then(|r| {
            let (label, after) = r.split_once("](")?;
            let (url, tail) = after.split_once(')')?;
            Some((Piece::Link(label, url), rest.len() - tail.len()))
        });
        let (piece, used, flanks) = match link {
            Some((link, used)) => (link, used, (false, false)),
            None if rest.starts_with("\\*") => (Piece::Text("*"), 2, (false, false)),
            None if rest.starts_with('*') => {
                let n = if rest.starts_with("**") { 2 } else { 1 };
                let (open, close) = (solid(rest[n..].chars().next()), solid(line[..at].chars().next_back()));
                (Piece::Star(&rest[..n], open || close), n, (open, close))
            }
            None => {
                let stop = rest.char_indices().skip(1).find(|(_, c)| matches!(c, '[' | '*' | '\\'));
                let end = stop.map_or(rest.len(), |(i, _)| i);
                (Piece::Text(&rest[..end]), end, (false, false))
            }
        };
        out.push((piece, flanks));
        at += used;
    }
    let mut used = vec![None; out.len()];
    for j in 0..out.len() {
        let (Piece::Star(mark, _), (_, true)) = out[j] else { continue };
        let opens =
            |i: &usize| used[*i].is_none() && matches!(out[*i], (Piece::Star(m, _), (true, _)) if m == mark);
        let Some(i) = (0..j.saturating_sub(1)).rev().find(opens) else { continue };
        (i + 1..j).for_each(|k| used[k] = used[k].or(Some(false)));
        (used[i], used[j]) = (Some(true), Some(true));
    }
    let resolve = |((piece, _), used)| match piece {
        Piece::Star("**", _) if used == Some(true) => Piece::Bold,
        Piece::Star(_, _) if used == Some(true) => Piece::Italic,
        piece => piece,
    };
    out.into_iter().zip(used).map(resolve).collect()
}

pub fn https(url: &str) -> bool {
    let host = url.strip_prefix("https://").and_then(|rest| rest.split(['/', '?', '#']).next());
    let clean = !url.chars().any(|c| c.is_whitespace() || c.is_control());
    url.len() <= 512 && host.is_some_and(|h| !h.is_empty()) && clean
}
