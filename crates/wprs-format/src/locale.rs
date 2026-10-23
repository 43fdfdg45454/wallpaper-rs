use crate::{Manifest, Text};
use std::collections::{BTreeMap, BTreeSet};
use wprs_scene::{Choice, Control, tag};

impl Manifest {
    pub fn key<'a>(&self, text: &'a Text) -> Option<&'a str> {
        match text {
            Text::Plain(plain) if self.v2() => plain.strip_prefix('@'),
            _ => None,
        }
    }

    pub fn resolve(&self, text: &Text) -> Text {
        let Some(key) = self.key(text) else { return text.clone() };
        let found =
            self.strings.iter().filter_map(|(lang, table)| Some((lang.clone(), table.get(key)?.clone())));
        Text::Localized(found.collect())
    }

    pub fn languages(&self, user: &[String]) -> Vec<String> {
        user.iter().cloned().chain(self.default_locale.clone()).collect()
    }

    pub fn pick(&self, text: &Text, user: &[String]) -> String {
        self.resolve(text).pick(&self.languages(user))
    }

    pub fn title_in(&self, user: &[String]) -> String {
        self.pick(&self.title, user)
    }

    pub fn locale_in(&self, user: &[String]) -> String {
        let found = self.locales().into_iter().map(|l| (l.clone(), l)).collect();
        Text::Localized(found).pick(&self.languages(user))
    }

    pub fn locales(&self) -> BTreeSet<String> {
        let mut found: BTreeSet<String> = self.strings.keys().map(|l| tag(l)).collect();
        for text in self.texts().into_iter().map(|(_, t)| t) {
            if let Text::Localized(map) = text {
                found.extend(map.keys().map(|l| tag(l)));
            }
        }
        found.extend(self.default_locale.as_deref().map(tag));
        found
    }

    pub fn localized(&self) -> Manifest {
        let mut out = self.clone();
        let fix = |t: &mut Text| *t = self.resolve(t);
        fix(&mut out.title);
        fix(&mut out.description);
        out.changes.iter_mut().for_each(fix);
        for property in out.properties.values_mut() {
            property.label.iter_mut().chain(property.help.iter_mut()).for_each(fix);
            if let Control::Combo { options, .. } = &mut property.control {
                options.iter_mut().for_each(|o| match o {
                    Choice::Labeled { label, .. } => fix(label),
                    Choice::Plain(_) => {}
                });
            }
        }
        out.presets.iter_mut().flat_map(|p| p.label.iter_mut()).for_each(fix);
        out
    }

    pub(crate) fn texts(&self) -> Vec<(String, &Text)> {
        let head = [("title", Some(&self.title)), ("description", Some(&self.description))];
        let head = head.into_iter().chain([("changes", self.changes.as_ref())]);
        let mut out: Vec<(String, &Text)> = head.filter_map(|(f, t)| Some((f.to_owned(), t?))).collect();
        for (id, p) in &self.properties {
            let own = [("label", p.label.as_ref()), ("help", p.help.as_ref())].into_iter();
            out.extend(own.filter_map(|(f, t)| Some((format!("properties.{id}.{f}"), t?))));
            if let Control::Combo { options, .. } = &p.control {
                out.extend(options.iter().filter_map(|o| match o {
                    Choice::Labeled { value, label } => {
                        Some((format!("properties.{id}.options.{value}"), label))
                    }
                    Choice::Plain(_) => None,
                }));
            }
        }
        out.extend(
            self.presets.iter().filter_map(|p| Some((format!("presets.{}", p.id), p.label.as_ref()?))),
        );
        out
    }
}

pub(crate) fn valid_tag(name: &str) -> bool {
    let mut parts = name.split('-');
    let first = parts
        .next()
        .is_some_and(|p| (2..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphabetic()));
    let rest: Vec<&str> = parts.collect();
    first
        && rest.len() <= 3
        && rest.iter().all(|p| (1..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
}

pub(crate) fn valid_key(key: &str) -> bool {
    (1..=64).contains(&key.len()) && key.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

pub(crate) fn has(map: &BTreeMap<String, String>, locale: &str) -> bool {
    map.keys().any(|k| tag(k) == tag(locale))
}
