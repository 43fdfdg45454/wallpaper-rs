use rquickjs::loader::{ImportAttributes, Loader, Resolver};
use rquickjs::module::Declared;
use rquickjs::{Ctx, Error, Module, Result};
use std::collections::HashMap;

pub(crate) const PRELUDE: &str = include_str!("prelude.js");
pub(crate) const MATH: &str = include_str!("math.js");

#[derive(Clone)]
pub(crate) struct Modules(pub(crate) HashMap<String, String>);

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
        let path = match name {
            "wprs" | "wprs/math" => return Ok(name.to_owned()),
            relative if relative.starts_with("./") || relative.starts_with("../") => joined(base, relative),
            plain => plain.to_owned(),
        };
        match self.0.contains_key(&path) {
            true => Ok(path),
            false => Err(Error::new_resolving_message(
                base,
                name,
                "only wprs, wprs/math and the scripts of the scene can be imported",
            )),
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
        let source = match name {
            "wprs" => PRELUDE,
            "wprs/math" => MATH,
            path => self.0.get(path).ok_or_else(|| Error::new_loading(path))?,
        };
        Module::declare(ctx.clone(), name, source)
    }
}
