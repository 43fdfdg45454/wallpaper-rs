use crate::names::{valid_id, valid_license, valid_name, valid_property, within};
use crate::{Error, Kind, Manifest, Result, SCHEMA, SCHEMA_V1, Text, require};

mod texts;
mod v2;

const FIRST: &str =
    "schema id version title description authors license kind entry preview tags properties presets";
const NEEDED: [(&str, &str); 4] = [
    ("content", "is required: a rating of general or mature and its reasons"),
    ("photosensitive", "is required: true when the package flashes"),
    ("origin", "is required: original, or derived with the works it comes from"),
    ("default_locale", "is required with strings or localized texts"),
];

type Rule<'a> = (&'a str, bool, &'a str);

const MAX_AUTHORS: usize = 64;
const MAX_LICENSE: usize = 256;

impl Manifest {
    pub fn check(&self) -> Result<()> {
        first(self.faults())
    }

    pub fn faults(&self) -> Vec<Error> {
        let schema = [SCHEMA, SCHEMA_V1].contains(&self.schema.as_str());
        let tags = self.tags.len() <= 32 && self.tags.iter().all(|t| within(t, 32));
        let authors = self.authors.len() <= MAX_AUTHORS && self.authors.iter().all(|a| within(a, 128));
        let license = self.license.chars().count() <= MAX_LICENSE && valid_license(&self.license);
        let mut out = broken(&[
            ("schema", schema, "expected wallpaper-rs.wpak/2"),
            ("id", valid_id(&self.id), "expected a lowercase reverse-DNS name like org.example.name"),
            ("version", semver::Version::parse(&self.version).is_ok(), "expected a semantic version"),
            ("authors", authors, "expected at most 64 authors of 1 to 128 characters each"),
            ("license", license, "expected an SPDX license expression of at most 256 characters"),
            ("tags", tags, "expected at most 32 tags of 1 to 32 characters"),
        ]);
        out.extend(if self.v2() { self.faults_v2() } else { self.check_v1().err().into_iter().collect() });
        let kind = self.check_kind().err();
        let shown = self
            .check_display()
            .err()
            .filter(|s| kind.as_ref().is_none_or(|k| k.to_string() != s.to_string()));
        out.extend(kind.into_iter().chain(shown));
        out.extend(self.property_faults());
        out
    }

    pub fn check_properties(&self) -> Result<()> {
        first(self.property_faults())
    }

    fn property_faults(&self) -> Vec<Error> {
        let mut out: Vec<Error> = self
            .properties
            .iter()
            .filter_map(|(name, property)| {
                let field = format!("properties.{name}");
                let valid = if self.v2() { valid_property(name) } else { valid_name(name) };
                require(valid, &field, "expected a letter followed by letters, digits or _").err().or_else(
                    || property.check().err().map(|reason| Error::Invalid { field, reason: reason.into() }),
                )
            })
            .collect();
        let mut properties = match self.kind {
            Kind::Video => self.video_properties(&Default::default(), None),
            _ => Default::default(),
        };
        properties.extend(self.properties.clone());
        let presets = wprs_scene::check_presets(&properties, &self.presets);
        out.extend(presets.err().map(|(field, reason)| Error::Invalid { field, reason: reason.into() }));
        out
    }

    pub fn missing(&self) -> Vec<Error> {
        let localized = self.texts().iter().any(|(_, t)| matches!(t, Text::Localized(_)));
        let locale = self.default_locale.is_none() && (localized || !self.strings.is_empty());
        let gone = [self.content.is_none(), self.photosensitive.is_none(), self.origin.is_none(), locale];
        let found = NEEDED.iter().zip(gone).filter(|(_, gone)| *gone && self.v2());
        found
            .map(|((field, reason), _)| Error::Invalid { field: field.to_string(), reason: (*reason).into() })
            .collect()
    }

    fn check_v1(&self) -> Result<()> {
        let plain = matches!((&self.title, &self.description), (Text::Plain(_), Text::Plain(_)));
        let json = serde_json::to_value(self).unwrap_or_default();
        let newer = json.as_object().is_some_and(|o| o.keys().any(|k| !FIRST.split(' ').any(|f| f == k)));
        let limits = self.properties.len() <= 64 && self.presets.len() <= 32;
        let kind = matches!(self.kind, Kind::Image | Kind::Video | Kind::Scene);
        let older =
            |p: &wprs_scene::Property| p.features().iter().all(|f| *f == "user_videos" && p.kind() == "file");
        let kinds = self.properties.values().all(older);
        apply(&[
            ("schema", plain && !newer, "localized texts and the newer fields need wallpaper-rs.wpak/2"),
            ("kind", kind, "web, preset and assets packages need wallpaper-rs.wpak/2"),
            ("title", within(&self.pick(&self.title, &[]), 128), "expected 1 to 128 characters"),
            ("properties", limits, "too many properties or presets"),
            ("properties", kinds, "the newer kinds and fields of properties need wallpaper-rs.wpak/2"),
        ])
    }
}

fn broken(rules: &[Rule]) -> Vec<Error> {
    rules.iter().filter_map(|(field, ok, reason)| require(*ok, field, reason).err()).collect()
}

fn apply(rules: &[Rule]) -> Result<()> {
    first(broken(rules))
}

fn first(faults: Vec<Error>) -> Result<()> {
    faults.into_iter().next().map_or(Ok(()), Err)
}
