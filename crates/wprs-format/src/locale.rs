use crate::{Manifest, Text};
use std::collections::{BTreeMap, BTreeSet};
use wprs_core::message::said;
use wprs_scene::{Choice, Control, tag};

pub const LOCALE: &str = "locale/";
pub const MAX_LOCALE_FILE: u64 = 512 << 10;
pub(crate) const MAX_STRINGS: usize = 512 << 10;
pub(crate) const LANGUAGES: &str =
    "at most 64 languages named like en or pt-BR, each in strings or in locale/<language>.json";
pub(crate) const KEYS: &str = "at most 2048 keys of [A-Za-z0-9_.-] per language, 512 KiB";
const UNREAD: &str = "stays an ordinary file; list locale_files in requires.features to read its texts";
pub(crate) const ENGINE_KEYS: &str = "keys that start with wprs. name texts of the engine";

pub const ENGINE: [(&str, &str, &str); 10] = [
    ("accent_color", "Accent color", "Color de acento"),
    ("background_color", "Background color", "Color de fondo"),
    ("brightness", "Brightness", "Brillo"),
    ("color", "Color", "Color"),
    ("density", "Density", "Densidad"),
    ("opacity", "Opacity", "Opacidad"),
    ("quality", "Quality", "Calidad"),
    ("size", "Size", "Tamaño"),
    ("speed", "Speed", "Velocidad"),
    ("volume", "Volume", "Volumen"),
];

pub fn engine(key: &str) -> Option<Text> {
    let (_, en, es) = ENGINE.iter().find(|(k, ..)| key.strip_prefix("wprs.") == Some(*k))?;
    Some(Text::Localized(BTreeMap::from([("en".into(), en.to_string()), ("es".into(), es.to_string())])))
}

fn language(path: &str) -> &str {
    path.strip_prefix(LOCALE).and_then(|n| n.strip_suffix(".json")).unwrap_or_default()
}

pub fn locale_file(path: &str) -> bool {
    path.strip_prefix(LOCALE).is_some_and(|n| n.ends_with(".json") && !n.contains('/'))
}

impl Manifest {
    pub fn key<'a>(&self, text: &'a Text) -> Option<&'a str> {
        match text {
            Text::Plain(plain) if self.v2() => plain.strip_prefix('@'),
            _ => None,
        }
    }

    pub fn resolve(&self, text: &Text) -> Text {
        let Some(key) = self.key(text) else { return text.clone() };
        if let Some(found) = engine(key) {
            return found;
        }
        let tables = self.strings.iter().chain(&self.string_files);
        let found = tables.filter_map(|(lang, table)| Some((lang.clone(), table.get(key)?.clone())));
        Text::Localized(found.collect())
    }

    pub fn table(&self, language: &str) -> Option<&BTreeMap<String, String>> {
        let mut tables = self.strings.iter().chain(&self.string_files);
        tables.find(|(l, _)| tag(l) == tag(language)).map(|(_, t)| t)
    }

    fn reads_files(&self) -> bool {
        self.v2() && self.features().iter().any(|f| f == "locale_files")
    }

    pub fn read_strings(
        &mut self,
        names: impl IntoIterator<Item = String>,
        mut read: impl FnMut(&str) -> crate::Result<Vec<u8>>,
    ) -> crate::Result<()> {
        if !self.reads_files() {
            return Ok(());
        }
        let mut names: Vec<String> = names.into_iter().filter(|n| locale_file(n)).collect();
        names.sort();
        crate::require(names.len() + self.strings.len() <= 64, "strings", LANGUAGES)?;
        let mut total = size(self.strings.values());
        for name in names {
            let language = language(&name).to_owned();
            let field = format!("{LOCALE}{language}.json");
            let table = serde_json::from_slice::<BTreeMap<String, String>>(&read(&name)?)
                .map_err(|e| crate::refuse(&field, said("format.json", "malformed JSON: %1", &[&e])))?;
            let known = valid_tag(&language) && !self.string_files.contains_key(&language);
            crate::require(known, &field, "expected one file per language named like locale/pt-BR.json")?;
            total += size([&table]);
            crate::require(total <= MAX_STRINGS, "strings", KEYS)?;
            self.string_files.insert(language, table);
        }
        Ok(())
    }

    pub fn unread_texts(&self, names: impl IntoIterator<Item = String>) -> Vec<String> {
        let skipped = self.v2() && !self.reads_files();
        let named = names.into_iter().filter(|n| skipped && locale_file(n) && valid_tag(language(n)));
        named.map(|n| format!("{n} {UNREAD}")).collect()
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
        let names = self.strings.keys().chain(self.string_files.keys());
        let mut found: BTreeSet<String> = names.map(|l| tag(l)).collect();
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
                    Choice::Labeled { value, label, .. } => {
                        let value = value.as_str().map_or_else(|| value.to_string(), String::from);
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

pub fn valid_tag(name: &str) -> bool {
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

pub(crate) fn size<'a>(tables: impl IntoIterator<Item = &'a BTreeMap<String, String>>) -> usize {
    tables.into_iter().flat_map(|t| t.iter()).map(|(k, v)| k.len() + v.len()).sum()
}

pub(crate) fn has(map: &BTreeMap<String, String>, locale: &str) -> bool {
    map.keys().any(|k| tag(k) == tag(locale))
}
