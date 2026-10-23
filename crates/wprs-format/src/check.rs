use crate::locale::{has, valid_key, valid_tag};
use crate::meta::{Content, Credit, Links, Origin};
use crate::names::{clean, safe, valid_id, valid_license, valid_name, valid_property, within};
use crate::{Display, Error, Kind, Manifest, Playback, Result, SCHEMA, SCHEMA_V1, Text, markup, require};
use std::collections::HashSet;
use wprs_scene::Control;

const MAX_STRINGS: usize = 512 << 10;
const FIRST: &str =
    "schema id version title description authors license kind entry preview tags properties presets";
const NEEDED: [(&str, &str); 4] = [
    ("content", "is required: a rating of general or mature and its reasons"),
    ("photosensitive", "is required: true when the package flashes"),
    ("origin", "is required: original, or derived with the works it comes from"),
    ("default_locale", "is required with strings or localized texts"),
];

type Rule<'a> = (&'a str, bool, &'a str);

impl Manifest {
    pub fn check(&self) -> Result<()> {
        let schema = [SCHEMA, SCHEMA_V1].contains(&self.schema.as_str());
        let tags = self.tags.len() <= 32 && self.tags.iter().all(|t| within(t, 32));
        apply(&[
            ("schema", schema, "expected wallpaper-rs.wpak/2"),
            ("id", valid_id(&self.id), "expected a lowercase reverse-DNS name like org.example.name"),
            ("version", semver::Version::parse(&self.version).is_ok(), "expected a semantic version"),
            ("authors", self.authors.iter().all(|a| within(a, 128)), "expected 1 to 128 characters each"),
            ("license", valid_license(&self.license), "expected an SPDX license expression"),
            ("tags", tags, "expected at most 32 tags of 1 to 32 characters"),
        ])?;
        if self.v2() { self.check_v2() } else { self.check_v1() }?;
        self.check_properties()
    }

    pub fn check_properties(&self) -> Result<()> {
        self.properties.iter().try_for_each(|(name, property)| {
            let field = format!("properties.{name}");
            let valid = if self.v2() { valid_property(name) } else { valid_name(name) };
            require(valid, &field, "expected a letter followed by letters, digits or _")?;
            property.check().map_err(|reason| Error::Invalid { field, reason })
        })?;
        let mut properties = match self.kind {
            Kind::Video => self.video_properties(&Default::default(), None),
            _ => Default::default(),
        };
        properties.extend(self.properties.clone());
        wprs_scene::check_presets(&properties, &self.presets)
            .map_err(|(field, reason)| Error::Invalid { field, reason })
    }

    pub fn missing(&self) -> Vec<Error> {
        let localized = self.texts().iter().any(|(_, t)| matches!(t, Text::Localized(_)));
        let locale = self.default_locale.is_none() && (localized || !self.strings.is_empty());
        let gone = [self.content.is_none(), self.photosensitive.is_none(), self.origin.is_none(), locale];
        let found = NEEDED.iter().zip(gone).filter(|(_, gone)| *gone && self.v2());
        found
            .map(|((field, reason), _)| Error::Invalid {
                field: field.to_string(),
                reason: reason.to_string(),
            })
            .collect()
    }

    fn check_v1(&self) -> Result<()> {
        let plain = matches!((&self.title, &self.description), (Text::Plain(_), Text::Plain(_)));
        let json = serde_json::to_value(self).unwrap_or_default();
        let newer = json.as_object().is_some_and(|o| o.keys().any(|k| !FIRST.split(' ').any(|f| f == k)));
        let limits = self.properties.len() <= 64 && self.presets.len() <= 32;
        let older =
            |p: &wprs_scene::Property| p.features().iter().all(|f| *f == "user_videos" && p.kind() == "file");
        let kinds = self.properties.values().all(older);
        apply(&[
            ("schema", plain && !newer, "localized texts and the newer fields need wallpaper-rs.wpak/2"),
            ("title", within(&self.pick(&self.title, &[]), 128), "expected 1 to 128 characters"),
            ("properties", limits, "too many properties or presets"),
            ("properties", kinds, "the newer kinds and fields of properties need wallpaper-rs.wpak/2"),
        ])
    }

    fn check_v2(&self) -> Result<()> {
        let (content, origin) = (self.content.as_ref(), self.origin.as_ref());
        let credits = self.credits.len() <= 64 && self.credits.iter().all(Credit::valid);
        let mut lower = HashSet::new();
        let unique = self.properties.keys().all(|id| lower.insert(id.to_lowercase()));
        let combo = |c: &Control| matches!(c, Control::Combo { options, .. } if options.len() > 64);
        let options = !self.properties.values().any(|p| combo(&p.control));
        let accept = |c: &Control| match c {
            Control::File { accept, .. } | Control::Folder { accept, .. } => {
                accept.iter().all(|a| ["image", "video"].contains(&a.as_str()))
            }
            _ => true,
        };
        let accepted = self.properties.values().all(|p| accept(&p.control));
        let video = self.kind == Kind::Video || (self.playback.is_none() && self.display.is_none());
        let limits = self.properties.len() <= 256 && self.presets.len() <= 64;
        let plain = self.authors.iter().chain(&self.tags).all(|t| safe(t));
        if let Some(missing) = self.missing().into_iter().next() {
            return Err(missing);
        }
        apply(&[
            ("authors", plain, "authors and tags have no < or >"),
            (
                "content.descriptors",
                content.is_none_or(Content::valid),
                "lists each reason once, one at least for mature",
            ),
            (
                "origin.derived_from",
                origin.is_none_or(Origin::valid),
                "derived packages credit 1 to 64 works by author, without < or >",
            ),
            ("preview", self.preview.is_none(), "is called previews.thumbnail in wallpaper-rs.wpak/2"),
            (
                "credits",
                credits,
                "at most 64, each with 1 to 256 paths, an SPDX license and an attribution without < or >",
            ),
            (
                "links",
                self.links.as_ref().is_none_or(Links::valid),
                "expected https:// addresses of 512 characters at most",
            ),
            ("properties", limits, "at most 256 properties and 64 presets"),
            ("properties", accepted, "files and folders accept image, video or both"),
            ("playback", video, "playback and display apply to video packages only"),
            (
                "playback",
                self.playback.as_ref().is_none_or(Playback::valid),
                "speed is 0.1 to 4, start and end 0 to 3600 s with end after start, volume 0 to 1",
            ),
            (
                "display",
                self.display.as_ref().is_none_or(Display::valid),
                "zoom is 1 to 8 and position two numbers of -1 to 1",
            ),
            (
                "properties",
                unique && options,
                "ids differ in more than case and combos have 64 options at most",
            ),
        ])?;
        self.check_texts()
    }

    fn check_texts(&self) -> Result<()> {
        let tables = self.strings.values();
        let size: usize = tables.clone().flat_map(|t| t.iter()).map(|(k, v)| k.len() + v.len()).sum();
        let keys = tables.clone().all(|t| t.len() <= 2048 && t.keys().all(|k| valid_key(k)));
        let values = tables.flat_map(|t| t.values()).all(|v| clean(v, 8000, true));
        let languages = self.strings.len() <= 64 && self.strings.keys().all(|l| valid_tag(l));
        let default = self.default_locale.as_deref();
        apply(&[
            ("strings", languages, "at most 64 languages named like en or pt-BR"),
            (
                "strings",
                keys && size <= MAX_STRINGS,
                "at most 2048 keys of [A-Za-z0-9_.-] per language, 512 KiB",
            ),
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
        if let Some(key) = self.key(text) {
            let known = default.and_then(|d| self.strings.get(d)).is_some_and(|t| t.contains_key(key));
            require(known, field, &format!("@{key} is not in the strings of the default language"))?;
        }
        let values: Vec<(String, String)> = match self.resolve(text) {
            Text::Plain(t) => vec![(String::new(), t)],
            Text::Localized(map) => {
                let named = map.keys().all(|l| valid_tag(l)) && default.is_none_or(|d| has(&map, d));
                require(named, field, "needs a text in the default language and languages like en or pt-BR")?;
                map.into_iter().map(|(language, t)| (format!(" in {language}"), t)).collect()
            }
        };
        let marked = ["description", "changes"].contains(&field);
        values.iter().try_for_each(|(language, value)| {
            require(field != "title" || !value.trim().is_empty(), field, &format!("has no text{language}"))?;
            let ok = clean(value, max, lines);
            require(ok, field, &format!("expected at most {max} characters without control characters"))?;
            require(marked || safe(value), field, "plain texts have no < or >")?;
            match marked {
                true => markup::check(value).map_err(|reason| Error::Invalid { field: field.into(), reason }),
                false => Ok(()),
            }
        })
    }
}

fn apply(rules: &[Rule]) -> Result<()> {
    rules.iter().try_for_each(|(field, ok, reason)| require(*ok, field, reason))
}
