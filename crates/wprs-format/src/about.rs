use crate::{Manifest, features};
use serde::Serialize;
use wprs_core::model::{About, Feature, Link, PackageCredit, Work};

fn name(value: impl Serialize) -> String {
    serde_json::to_value(value).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
}

impl Manifest {
    pub fn about(&self, languages: &[String]) -> About {
        let pick = |t| self.pick(t, languages);
        let origin = self.origin.as_ref();
        let work = |a: &crate::Ancestor| Work {
            author: a.author.clone(),
            title: a.title.clone(),
            url: a.url.clone(),
            license: a.license.clone(),
        };
        let credit = |c: &crate::Credit| PackageCredit {
            paths: c.paths.clone(),
            license: c.license.clone(),
            attribution: c.attribution.clone(),
            url: c.url.clone(),
        };
        let links = self.links.iter().flat_map(|l| l.all());
        let links = links.filter_map(|(kind, url)| Some(Link { kind: kind.into(), url: url?.clone() }));
        let feature = |f: &String| Feature { name: f.clone(), why: features::explain(f) };
        About {
            schema: self.schema.clone(),
            title: pick(&self.title),
            description: pick(&self.description),
            changes: self.changes.as_ref().map(pick).unwrap_or_default(),
            authors: self.authors.clone(),
            license: self.license.clone(),
            content: self.content.as_ref().map(|c| name(c.rating)),
            descriptors: self.content.iter().flat_map(|c| &c.descriptors).map(name).collect(),
            photosensitive: self.photosensitive,
            origin: origin.map(|o| name(o.kind)),
            derived_from: origin.iter().flat_map(|o| &o.derived_from).map(work).collect(),
            credits: self.credits.iter().map(credit).collect(),
            links: links.collect(),
            inputs: self.inputs.iter().map(|i| i.name().into()).collect(),
            features: self.features().iter().map(feature).collect(),
            languages: self.locales().into_iter().collect(),
        }
    }
}
