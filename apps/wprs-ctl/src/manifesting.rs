use crate::steps::value;
use clap::Subcommand;
use serde_json::{Map, Value, json};
use wprs_editor::actions::ManifestEdit as M;

#[derive(Subcommand, Debug)]
pub enum Meta {
    #[command(
        about = "Say whether the package is original, derived from other works, imported or from a provider"
    )]
    Origin {
        #[arg(help = "original, derived, imported or provider", value_parser = ["original", "derived", "imported", "provider"])]
        kind: String,
        #[arg(
            long = "derived-from",
            value_name = "AUTHOR",
            help = "Author of a work it is based on, or a JSON object with author, title, url and license"
        )]
        derived_from: Vec<String>,
        #[arg(long, requires_all = ["item_url", "credit"], help = "Name of the provider the work comes from")]
        provider: Option<String>,
        #[arg(long, requires = "provider", help = "Page of the work at the provider")]
        item_url: Option<String>,
        #[arg(long, requires = "provider", help = "Credit the provider asks for")]
        credit: Option<String>,
        #[arg(long, requires = "provider", help = "Terms of the provider")]
        terms_url: Option<String>,
        #[arg(long, help = "Where the original files are")]
        source_url: Option<String>,
    },
    #[command(about = "Set the license as an SPDX expression such as CC-BY-4.0")]
    License {
        #[arg(help = "SPDX expression of at most 256 characters, such as CC-BY-4.0 or MIT OR Apache-2.0")]
        license: String,
    },
    #[command(about = "Replace the tags, or add or remove some")]
    Tags {
        #[arg(help = "Tags of 1 to 32 characters, at most 32; the list that replaces the old one")]
        tags: Vec<String>,
        #[arg(long, conflicts_with = "remove", help = "Add the tags to the ones there are")]
        add: bool,
        #[arg(long, help = "Remove the tags from the ones there are")]
        remove: bool,
    },
    #[command(about = "Replace the authors; none removes them")]
    Authors {
        #[arg(help = "Names of 1 to 128 characters, at most 64, or none to remove them")]
        authors: Vec<String>,
    },
    #[command(
        about = "Set how the package fills the screen, its accent color and whether it spans every screen"
    )]
    Display {
        #[arg(long, value_parser = ["fill", "fit", "stretch", "center", "tile", "span"], help = "How a picture or video fills the screen")]
        fit: Option<String>,
        #[arg(allow_negative_numbers = true, long, help = "Zoom of a picture, from 1 to 8")]
        zoom: Option<f64>,
        #[arg(allow_hyphen_values = true, long, value_name = "X,Y", value_parser = crate::sowed::pair, help = "Center of a picture, two numbers from -1 to 1")]
        position: Option<[f64; 2]>,
        #[arg(long, help = "Color as #rrggbb")]
        background: Option<String>,
        #[arg(long, value_parser = ["per-output", "all-outputs"], help = "Draw on each screen, or once across all of them")]
        span: Option<String>,
        #[arg(long, value_parser = ["none", "horizontal", "vertical"], help = "Mirror a picture or video")]
        flip: Option<String>,
        #[arg(long, help = "Color as #rrggbb, or prop:ID to take it from a property")]
        accent: Option<String>,
        #[arg(long, value_name = "SETTING", help = "Remove a setting")]
        clear: Vec<String>,
    },
}

pub(crate) fn edit(what: Meta) -> M {
    match what {
        Meta::Origin { kind, derived_from, provider, item_url, credit, terms_url, source_url } => {
            let ancestor = |a: &String| match value(a) {
                Value::String(author) => json!({ "author": author }),
                other => other,
            };
            let provider = provider.map(|name| {
                let mut p = json!({ "name": name, "item_url": item_url, "credit": credit });
                terms_url.iter().for_each(|t| p["terms_url"] = json!(t));
                p
            });
            M::Origin {
                kind: serde_json::from_value(json!(kind)).unwrap_or_default(),
                derived_from: derived_from.iter().map(ancestor).collect(),
                provider,
                source_url,
            }
        }
        Meta::License { license } => M::License { license },
        Meta::Tags { tags, add, remove } => M::Tags { tags, add, remove },
        Meta::Authors { authors } => {
            M::Authors { authors: authors.into_iter().filter(|a| a != "none").collect() }
        }
        Meta::Display { fit, zoom, position, background, span, flip, accent, clear } => {
            let mut values = Map::new();
            let texts = [("fit", fit), ("background", background), ("span", span), ("flip", flip)];
            texts
                .into_iter()
                .for_each(|(k, v)| v.into_iter().for_each(|v| drop(values.insert(k.into(), json!(v)))));
            zoom.iter().for_each(|z| drop(values.insert("zoom".into(), json!(z))));
            position.iter().for_each(|p| drop(values.insert("position".into(), json!(p))));
            let accent = accent.map(|a| match a.strip_prefix("prop:") {
                Some(id) => json!({ "prop": id }),
                None => json!(a),
            });
            accent.into_iter().for_each(|a| drop(values.insert("accent".into(), a)));
            M::Display { values, clear }
        }
    }
}
