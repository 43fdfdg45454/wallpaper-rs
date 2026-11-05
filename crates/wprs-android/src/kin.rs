use crate::library::with;
use crate::variant::{based, matching, needed, present};
use crate::{Error, repos, said, store};
use serde::Deserialize;
use serde_json::{Value, json};
use wprs_format::Dependency;
use wprs_library::Item;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(crate) enum Request {
    Kin { uid: String, languages: Option<Vec<String>> },
}

pub(crate) fn request(request: Request) -> Result<Value, Error> {
    match request {
        Request::Kin { uid, languages } => kin(&uid, &languages.unwrap_or_default()),
    }
}

fn named(item: Item) -> Value {
    json!({ "uid": item.uid, "title": item.title })
}

fn source(uid: &str, base: &Dependency) -> Option<String> {
    let own = repos::installed().remove(uid).map(|(repo, _)| repo);
    let own = own.filter(|repo| matching(Some(repo), base).is_ok());
    own.or_else(|| matching(None, base).ok().and_then(|spec| spec.split_once('/').map(|(r, _)| r.to_owned())))
}

pub(crate) fn kin(uid: &str, languages: &[String]) -> Result<Value, Error> {
    let (item, base, mut found, mut presets) = with(|lib| {
        let item = lib.item(uid)?;
        let base = based(&item)?;
        let found: Vec<Item> =
            base.as_ref().map(|b| present(lib, b)).transpose()?.flatten().into_iter().collect();
        let own = item.package.as_ref().filter(|p| p.kind != "preset").map(|p| p.id.clone());
        let presets = own.map(|id| lib.variants(&id)).transpose()?.unwrap_or_default();
        Ok((item, base, found, presets))
    })?;
    with(|lib| Ok(lib.localize(&mut found, languages).and(lib.localize(&mut presets, languages))?))?;
    let base = base.map(|base| {
        let repo = found.is_empty().then(|| source(&item.uid, &base)).flatten();
        json!({ "id": base.id, "version": base.version, "item": found.pop().map(named), "repo": repo })
    });
    Ok(json!({ "base": base, "presets": presets.into_iter().map(named).collect::<Vec<_>>() }))
}

pub(crate) fn install_base(uid: &str) -> Result<Value, Error> {
    let (item, base) = with(|lib| {
        let item = lib.item(uid)?;
        let base = based(&item)?;
        Ok((item, base))
    })?;
    let Some(base) = base else { return Ok(json!({ "item": null, "installed": [] })) };
    let missing = || said("android.no_release", "no release of %1 matches %2", &[&base.id, &base.version]);
    let repo = source(uid, &base).ok_or_else(missing)?;
    let installed = needed(&repo, &item.path, 0);
    store::changed();
    let found = with(|lib| present(lib, &base))?.ok_or_else(missing)?;
    Ok(json!({ "item": found.uid, "installed": installed }))
}
