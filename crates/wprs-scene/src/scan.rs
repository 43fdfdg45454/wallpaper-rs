use crate::{Scene, Script, ScriptApi, sites};

const FIRST: [&str; 8] = ["engine", "time", "props", "audio", "input", "scene", "timeline", "particles"];
const GLOBALS: [&str; 5] = ["setTimeout", "setInterval", "clearTimeout", "clearInterval", "console"];
const STARTS: [&str; 5] = ["init", "destroy", "resize", "properties", "settings"];

#[derive(Debug, PartialEq)]
enum Token<'a> {
    Word(&'a str),
    Text(String),
    Mark(char),
}

fn tokens(source: &str) -> Vec<Token<'_>> {
    let (bytes, mut out, mut i) = (source.as_bytes(), Vec::new(), 0);
    let until =
        |from: usize, end: &str| source[from..].find(end).map_or(source.len(), |at| from + at + end.len());
    while i < bytes.len() {
        let c = bytes[i];
        if source[i..].starts_with("//") {
            i = until(i, "\n");
        } else if source[i..].starts_with("/*") {
            i = until(i + 2, "*/");
        } else if matches!(c, b'"' | b'\'' | b'`') {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != c {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            out.push(Token::Text(source[i + 1..j.min(bytes.len())].to_owned()));
            i = j + 1;
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' {
            let end = source[i..].find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'));
            let end = end.map_or(source.len(), |e| i + e);
            out.push(Token::Word(&source[i..end]));
            i = end;
        } else {
            if !c.is_ascii_whitespace() {
                out.push(Token::Mark(c as char));
            }
            i += source[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

fn api(name: &str) -> Option<ScriptApi> {
    Some(match name {
        "audio" => ScriptApi::Audio,
        "input" => ScriptApi::Pointer,
        "particles" => ScriptApi::Particles,
        "setTimeout" | "setInterval" | "clearTimeout" | "clearInterval" => ScriptApi::Timers,
        "console" => ScriptApi::Console,
        "shared" => ScriptApi::Shared,
        "sound" => ScriptApi::Sounds,
        _ => return None,
    })
}

fn braced<'a>(list: &[Token<'a>]) -> Vec<(&'a str, &'a str)> {
    let inner = list.iter().skip_while(|t| **t != Token::Mark('{')).skip(1);
    let inner: Vec<&Token> = inner.take_while(|t| **t != Token::Mark('}')).collect();
    let words = |part: &[&Token<'a>]| -> Vec<&'a str> {
        part.iter().filter_map(|t| if let Token::Word(w) = t { Some(*w) } else { None }).collect()
    };
    let parts = inner.split(|t| **t == Token::Mark(','));
    parts.map(words).filter_map(|w| Some((*w.first()?, *w.last()?))).collect()
}

fn modules<'a, 'b>(all: &'b [Token<'a>]) -> Vec<(&'b [Token<'a>], &'b str)> {
    let mut found = Vec::new();
    for (i, t) in all.iter().enumerate() {
        let (Token::Word("from"), Some(Token::Text(m))) = (t, all.get(i + 1)) else { continue };
        let start = all[..i].iter().rposition(|t| matches!(t, Token::Word("import" | "export")));
        found.extend(start.filter(|s| i - s <= 256).map(|s| (&all[s + 1..i], m.as_str())));
    }
    found
}

pub fn imported(source: &str) -> Vec<ScriptApi> {
    let all = tokens(source);
    let mut found = Vec::new();
    for (names, module) in modules(&all) {
        match module {
            "wprs/math" => found.push(ScriptApi::Math),
            "wprs" if names.contains(&Token::Mark('*')) => {
                found.extend([ScriptApi::Audio, ScriptApi::Pointer])
            }
            "wprs" => found.extend(braced(names).into_iter().filter_map(|(name, _)| api(name))),
            _ => found.push(ScriptApi::Shared),
        }
    }
    let loose = all
        .windows(2)
        .any(|w| w[0] == Token::Word("import") && matches!(w[1], Token::Text(_) | Token::Mark('(')));
    found.extend(loose.then_some(ScriptApi::Shared));
    found.sort();
    found.dedup();
    found
}

pub fn beyond_first(source: &str) -> Vec<String> {
    let all = tokens(source);
    let mut found: Vec<String> = Vec::new();
    for (names, module) in modules(&all) {
        match module {
            "wprs" => found.extend(
                braced(names).into_iter().map(|(n, _)| n).filter(|n| !FIRST.contains(n)).map(String::from),
            ),
            other => found.push(format!("imports of {other}")),
        }
    }
    for (i, t) in all.iter().enumerate() {
        let (before, next) = (i.checked_sub(1).map(|j| &all[j]), all.get(i + 1));
        match t {
            Token::Word(w) if GLOBALS.contains(w) && before != Some(&Token::Mark('.')) => {
                found.push((*w).into())
            }
            Token::Word("import") if matches!(next, Some(Token::Text(_) | Token::Mark('('))) => {
                found.push("imports of other modules".into())
            }
            Token::Word("export") => {
                let rest = &all[i + 1..all.len().min(i + 256)];
                let named = rest.iter().skip_while(|t| {
                    matches!(
                        t,
                        Token::Word("async" | "function" | "const" | "let" | "var" | "class")
                            | Token::Mark('*')
                    )
                });
                let names = match (rest.first(), named.clone().next()) {
                    (Some(Token::Mark('{')), _) => braced(rest).into_iter().map(|(_, n)| n).collect(),
                    (Some(Token::Word(_)), Some(Token::Word(name))) => vec![*name],
                    _ => Vec::new(),
                };
                found.extend(
                    names.into_iter().filter(|n| STARTS.contains(n)).map(|n| format!("its {n} export")),
                );
            }
            _ => {}
        }
    }
    found.sort();
    found.dedup();
    found
}

impl Script {
    pub fn declares(&self) -> bool {
        !self.uses.is_empty() || !self.events.is_empty() || !self.params.is_empty()
    }

    pub fn effective(&self, source: &str) -> Vec<ScriptApi> {
        if self.declares() { self.uses.clone() } else { imported(source) }
    }
}

impl Scene {
    pub fn first_interface(&self) -> Vec<String> {
        let bound = sites(self);
        let newer = |s: &&Script| s.declares() || bound.iter().any(|b| b.script == s.path);
        self.scripts.iter().filter(|s| !newer(s)).map(|s| s.path.clone()).collect()
    }

    pub fn undeclared(&self, read: impl Fn(&str) -> Option<String>) -> Option<(usize, String)> {
        let first = self.first_interface();
        self.scripts.iter().enumerate().filter(|(_, s)| first.contains(&s.path)).find_map(|(i, s)| {
            let used = beyond_first(&read(&s.path)?);
            (!used.is_empty()).then(|| {
                let used = used.join(", ");
                (
                    i,
                    format!(
                        "{} declares nothing in scene.json but uses {used}; declare its events and uses",
                        s.path
                    ),
                )
            })
        })
    }
}
