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
        self.check_kind()?;
        self.check_properties()
    }

    pub fn check_properties(&self) -> Result<()> {
        self.properties.iter().try_for_each(|(name, property)| {
            let field = format!("properties.{name}");
            let valid = if self.v2() { valid_property(name) } else { valid_name(name) };
            require(valid, &field, "expected a letter followed by letters, digits or _")?;
            property.check().map_err(|reason| Error::Invalid { field, reason: reason.into() })
        })?;
        let mut properties = match self.kind {
            Kind::Video => self.video_properties(&Default::default(), None),
            _ => Default::default(),
        };
        properties.extend(self.properties.clone());
        wprs_scene::check_presets(&properties, &self.presets)
            .map_err(|(field, reason)| Error::Invalid { field, reason: reason.into() })
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

fn apply(rules: &[Rule]) -> Result<()> {
    rules.iter().try_for_each(|(field, ok, reason)| require(*ok, field, reason))
}
