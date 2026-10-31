use super::apply;
use crate::locale::{ENGINE_KEYS as ENGINE, KEYS, LANGUAGES, MAX_STRINGS, has, size, valid_key, valid_tag};
use crate::names::{clean, safe};
use crate::{Error, Manifest, Result, Text, markup, require};
use wprs_core::message::said;
use wprs_scene::tag;

impl Manifest {
    pub(super) fn check_texts(&self) -> Result<()> {
        let tables = self.strings.values().chain(self.string_files.values());
        let size = size(tables.clone());
        let keys = tables.clone().all(|t| t.len() <= 2048 && t.keys().all(|k| valid_key(k)));
        let own = tables.clone().flat_map(|t| t.keys()).all(|k| !k.starts_with("wprs."));
        let values = tables.flat_map(|t| t.values()).all(|v| clean(v, 8000, true));
        let names = self.strings.keys().chain(self.string_files.keys());
        let once = self.string_files.keys().all(|l| !self.strings.keys().any(|s| tag(s) == tag(l)));
        let languages = names.clone().count() <= 64 && names.clone().all(|l| valid_tag(l)) && once;
        let default = self.default_locale.as_deref();
        apply(&[
            ("strings", languages, LANGUAGES),
            ("strings", keys && size <= MAX_STRINGS, KEYS),
            ("strings", own, ENGINE),
            ("strings", values, "texts of at most 8000 characters without control characters"),
            ("default_locale", default.is_none_or(valid_tag), "expected a language such as en or pt-BR"),
        ])?;
        self.texts().into_iter().try_for_each(|(field, text)| self.check_text(&field, text))
    }

    fn check_text(&self, field: &str, text: &Text) -> Result<()> {
        let (max, lines) = match field {
            "title" => (128, false),
            "description" => (8000, true),
            "changes" => (2000, true),
            _ => (1024, true),
        };
        let default = self.default_locale.as_deref();
        if let Some(key) = self.key(text).filter(|k| k.starts_with("wprs.")) {
            let reason = || {
                crate::refuse(field, said("format.engine_text", "@%1 is not a text of the engine", &[&key]))
            };
            return crate::ensure(crate::locale::engine(key).is_some(), reason);
        }
        if let Some(key) = self.key(text) {
            let known = default.and_then(|d| self.table(d)).is_some_and(|t| t.contains_key(key));
            let reason =
                || said("format.unknown_key", "@%1 is not in the strings of the default language", &[&key]);
            crate::ensure(known, || crate::refuse(field, reason()))?;
        }
        let values: Vec<(String, String)> = match self.resolve(text) {
            Text::Plain(t) => vec![(String::new(), t)],
            Text::Localized(map) => {
                let named = map.keys().all(|l| valid_tag(l)) && default.is_none_or(|d| has(&map, d));
                require(named, field, "needs a text in the default language and languages like en or pt-BR")?;
                map.into_iter().collect()
            }
        };
        let marked = ["description", "changes"].contains(&field);
        values.iter().try_for_each(|(language, value)| {
            let empty = || match language.as_str() {
                "" => said("format.no_text", "has no text", &[]),
                language => said("format.no_text_in", "has no text in %1", &[&language]),
            };
            crate::ensure(field != "title" || !value.trim().is_empty(), || crate::refuse(field, empty()))?;
            let long = || {
                said("format.too_long", "expected at most %1 characters without control characters", &[&max])
            };
            crate::ensure(clean(value, max, lines), || crate::refuse(field, long()))?;
            require(marked || safe(value), field, "plain texts have no < or >")?;
            match marked {
                true => markup::check(value)
                    .map_err(|reason| Error::Invalid { field: field.into(), reason: reason.into() }),
                false => Ok(()),
            }
        })
    }
}
