use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{Ctx, Error, Module, Result};
use std::collections::{HashMap, HashSet};

const PARTS: [(&str, &str); 10] = [
    ("wprs", include_str!("js/prelude.js")),
    ("wprs/math", include_str!("js/math.js")),
    ("wprs:core", include_str!("js/core.js")),
    ("wprs:values", include_str!("js/values.js")),
    ("wprs:base", include_str!("js/base.js")),
    ("wprs:extras", include_str!("js/extras.js")),
    ("wprs:layers", include_str!("js/layers.js")),
    ("wprs:media", include_str!("js/media.js")),
    ("wprs:life", include_str!("js/life.js")),
    ("wprs:first", include_str!("js/first.js")),
];

#[derive(Clone)]
pub(crate) struct Modules {
    pub(crate) sources: HashMap<String, String>,
    pub(crate) first: HashSet<String>,
}

fn part(name: &str) -> Option<&'static str> {
    PARTS.iter().find(|(n, _)| *n == name).map(|(_, s)| *s)
}

fn joined(base: &str, name: &str) -> String {
    let mut parts: Vec<&str> = base.rsplit_once('/').map_or(Vec::new(), |(dir, _)| dir.split('/').collect());
    for part in name.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

impl Resolver for Modules {
    fn resolve<'js>(
        &mut self,
        _: &Ctx<'js>,
        base: &str,
        name: &str,
        _: Option<ImportAttributes<'js>>,
    ) -> Result<String> {
        let refused = |why: &str| Err(Error::new_resolving_message(base, name, why));
        let path = match name {
            "wprs" if self.first.contains(base) => return Ok("wprs:first".into()),
            "wprs" | "wprs/math" if !self.first.contains(base) => return Ok(name.to_owned()),
            inner if part(base).is_some() && part(inner).is_some() => return Ok(name.to_owned()),
            _ if self.first.contains(base) => {
                return refused("a module that declares nothing in scene.json imports only wprs");
            }
            relative if relative.starts_with("./") || relative.starts_with("../") => joined(base, relative),
            plain => plain.to_owned(),
        };
        match self.sources.contains_key(&path) && part(&path).is_none() {
            true => Ok(path),
            false => refused("only wprs, wprs/math and the scripts of the scene can be imported"),
        }
    }
}

impl Loader for Modules {
    fn load<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        name: &str,
        _: Option<ImportAttributes<'js>>,
    ) -> Result<Module<'js, Declared>> {
        let source = match part(name) {
            Some(source) => source,
            None => self.sources.get(name).ok_or_else(|| Error::new_loading(name))?,
        };
        Module::declare(ctx.clone(), name, source)
    }
}
