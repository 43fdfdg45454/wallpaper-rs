use crate::locale::{catalog, languages};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::sync::LazyLock;

const CATALOGS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));
static CATALOG: LazyLock<HashMap<String, String>> = LazyLock::new(|| catalog(CATALOGS, &languages()));

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub id: &'static str,
    pub args: Vec<String>,
    #[serde(skip)]
    pub text: &'static str,
    #[serde(skip)]
    pub texts: Vec<usize>,
    #[serde(skip)]
    pub nested: Vec<(usize, Message)>,
}

pub fn said(id: &'static str, text: &'static str, args: &[&dyn fmt::Display]) -> Message {
    Message {
        id,
        text,
        args: args.iter().map(ToString::to_string).collect(),
        texts: Vec::new(),
        nested: Vec::new(),
    }
}

pub fn render(text: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('%') {
        out.push_str(&rest[..at]);
        let digit = rest[at + 1..].chars().next().and_then(|c| c.to_digit(10)).filter(|d| *d > 0);
        match digit.and_then(|d| args.get(d as usize - 1)) {
            Some(arg) => out.push_str(arg),
            None => out.push_str(&rest[at..at + 1 + usize::from(digit.is_some())]),
        }
        rest = &rest[at + 1 + usize::from(digit.is_some())..];
    }
    out + rest
}

pub fn tr(text: &str) -> String {
    CATALOG.get(text).cloned().unwrap_or_else(|| text.to_owned())
}

pub fn tr_in(languages: &[String]) -> impl Fn(&str) -> String + use<> {
    let found = catalog(CATALOGS, languages);
    move |text| found.get(text).cloned().unwrap_or_else(|| text.to_owned())
}

impl Message {
    pub fn texts(mut self, which: &[usize]) -> Self {
        self.texts.extend(which);
        self
    }

    pub fn with(mut self, at: usize, inner: Message) -> Self {
        if let Some(arg) = self.args.get_mut(at - 1) {
            *arg = inner.to_string();
        }
        self.nested.push((at, inner));
        self
    }

    pub fn told_args(&self, tr: &dyn Fn(&str) -> String) -> Vec<String> {
        let arg = |(i, a): (usize, &String)| match self.nested.iter().find(|(at, _)| *at == i + 1) {
            Some((_, inner)) => inner.told(tr),
            None if self.texts.contains(&(i + 1)) => tr(a),
            None => a.clone(),
        };
        self.args.iter().enumerate().map(arg).collect()
    }

    pub fn told(&self, tr: &dyn Fn(&str) -> String) -> String {
        render(&tr(self.text), &self.told_args(tr))
    }

    pub fn local(&self) -> String {
        self.told(&tr)
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&render(self.text, &self.args))
    }
}

impl std::error::Error for Message {}

impl From<Message> for String {
    fn from(message: Message) -> String {
        message.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_put_in_one_pass() {
        assert_eq!(render("%1 and %2", &["%2".into(), "b".into()]), "%2 and b");
        assert_eq!(render("100% of %3 %x", &["a".into()]), "100% of %3 %x");
        let inner = said("inner", "keep %1", &[&"x"]);
        let outer = said("outer", "%1: %2", &[&"/a", &"?"]).with(2, inner).texts(&[1]);
        assert_eq!(outer.to_string(), "/a: keep x");
        let upper = |t: &str| t.to_uppercase();
        assert_eq!(outer.told(&upper), "/A: KEEP x");
        assert_eq!(
            serde_json::to_value(&outer).unwrap(),
            serde_json::json!({"id": "outer", "args": ["/a", "keep x"]})
        );
    }
}
