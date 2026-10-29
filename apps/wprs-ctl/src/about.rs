use crate::show::{clean, list};
use wprs_core::markup::plain;
use wprs_core::model::{About, PackageInfo};
use wprs_format::SCHEMA_V1;

pub fn fields(about: &About, package: Option<&PackageInfo>) -> Vec<(&'static str, Option<String>)> {
    let confirmed = |what: &str| package.is_some_and(|p| p.confirmed.iter().any(|c| c == what));
    let reasons = Some(list(&about.descriptors)).filter(|r| !r.is_empty()).map(|r| format!(" ({r})"));
    let content = about.content.clone().unwrap_or("unrated".into()) + &reasons.unwrap_or_default();
    let flashing = about.photosensitive.map(|on| match (on, confirmed("photosensitive")) {
        (true, true) => "yes, allowed".into(),
        (true, false) => "yes, asks before it is shown".into(),
        (false, _) => "no".into(),
    });
    let work = |w: &wprs_core::model::Work| {
        w.title.as_ref().map_or(w.author.clone(), |t| format!("{t} by {}", w.author))
    };
    let works = list(about.derived_from.iter().map(work));
    let origin = about.origin.clone().map(|o| if works.is_empty() { o } else { format!("{o} from {works}") });
    let source = |c: &wprs_core::model::PackageCredit| c.source.as_ref().map(|s| format!(" from {s}"));
    let credits = about.credits.iter().map(|c| {
        format!("{}: {} ({}){}", list(&c.paths), c.attribution, c.license, source(c).unwrap_or_default())
    });
    let needs = about.features.iter().map(|f| format!("{} ({})", f.name, f.why));
    vec![
        ("content", package.is_some().then_some(content)),
        (
            "mature shown",
            (about.content.as_deref() == Some("mature")).then(|| confirmed("mature").to_string()),
        ),
        ("photosensitive", flashing),
        ("origin", origin),
        ("provider", about.provider.clone()),
        ("source", about.source_url.clone()),
        ("authors", Some(list(&about.authors))),
        ("credits", Some(list(credits))),
        ("links", Some(list(about.links.iter().map(|l| format!("{} {}", l.kind, l.url))))),
        ("inputs", Some(list(&about.inputs))),
        ("needs", Some(list(needs))),
        ("made for", Some(list(&about.platforms))),
        ("needs packages", Some(list(&about.packages))),
        ("languages", Some(list(&about.languages))),
        ("schema", Some(about.schema.clone())),
    ]
}

pub fn texts(about: &About) -> String {
    let marked = |text: &str| if about.schema == SCHEMA_V1 { text.to_owned() } else { plain(text) };
    let block = |head: &str, text: &str| match text.trim() {
        "" => String::new(),
        text => format!("\n{head}\n{}\n", marked(text).lines().map(clean).collect::<Vec<_>>().join("\n")),
    };
    block("Description:", &about.description) + &block("Changes:", &about.changes)
}
