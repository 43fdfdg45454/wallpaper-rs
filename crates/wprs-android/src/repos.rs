use crate::said;
use crate::state::{read, update};
use crate::{Error, library, store};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use wprs_format::Anchor;
use wprs_providers::HttpFetch;
use wprs_sync::{Ids, Kept, Local, Replica, View};

const DIR: &str = "repos";
const ITEMS: &str = "items.json";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct Offer {
    pub(crate) id: String,
    pub(crate) repo_id: String,
    pub(crate) name: String,
    pub(crate) url: String,
    pub(crate) fingerprints: Vec<String>,
}

pub(crate) fn dir() -> Result<PathBuf, Error> {
    Ok(store::dir()?.join(DIR))
}

fn now() -> i64 {
    library::now()
}

fn shared(url: &str) -> bool {
    let loopback = |u: &reqwest::Url| {
        let host = u.host_str().unwrap_or_default().trim_matches(['[', ']']);
        host == "localhost" || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
    };
    reqwest::Url::parse(url).is_ok_and(|u| u.scheme() == "https" || u.scheme() == "http" && loopback(&u))
}

fn field(view: &View, name: &str) -> String {
    view.get(name).and_then(Value::as_str).unwrap_or_default().to_owned()
}

pub(crate) fn installed() -> BTreeMap<String, (String, String)> {
    let file = dir().ok().and_then(|d| std::fs::read(d.join(ITEMS)).ok());
    file.and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

pub(crate) fn views(
    ids: &Ids,
    replica: &Replica,
    present: &BTreeSet<String>,
    local: &mut Local,
    kept: &mut Kept,
) -> Result<(), Error> {
    let found = wprs_format::repos(&dir()?, now()).unwrap_or_default();
    for repo in found.into_iter().filter(|r| shared(&r.url) && r.broken.is_none()) {
        let key = format!("repos/{}", ids.named("repo", &repo.repo_id));
        let fields = [
            ("url", json!(repo.url)),
            ("repo_id", json!(repo.repo_id)),
            ("name", json!(repo.name)),
            ("fingerprints", json!(repo.fingerprints)),
        ];
        let view: View = fields.map(|(k, v)| (k.to_owned(), v)).into();
        if replica.bases.contains_key(&key) {
            let held = kept.entry(key.clone()).or_default();
            for (field, value) in view.iter().filter(|(field, _)| *field != "repo_id") {
                held.entry(field.clone()).or_insert_with(|| value.clone());
            }
        }
        local.views.insert(key.clone(), view);
        local.repos.insert(key);
    }
    let others = replica.bases.iter().filter(|(key, _)| key.starts_with("repos/") && !present.contains(*key));
    let others: Vec<(String, View)> = others
        .filter(|(key, _)| !local.views.contains_key(*key))
        .filter_map(|(key, (_, doc))| Some((key.clone(), doc.view()?)))
        .collect();
    local.views.extend(others);
    Ok(())
}

pub(crate) fn offers(local: &Local, replica: &Replica) -> Vec<Offer> {
    let declined = read(|s| s.declined.clone());
    let waiting = replica.bases.iter().filter(|(key, _)| key.starts_with("repos/"));
    let waiting = waiting.filter(|(key, _)| !local.repos.contains(*key) && !declined.contains(*key));
    let offer = |(key, view): (&String, View)| Offer {
        id: key.trim_start_matches("repos/").to_owned(),
        repo_id: field(&view, "repo_id"),
        name: field(&view, "name"),
        url: field(&view, "url"),
        fingerprints: serde_json::from_value(view.get("fingerprints").cloned().unwrap_or_default())
            .unwrap_or_default(),
    };
    let found = waiting.filter_map(|(key, (_, doc))| Some((key, doc.view()?)));
    found.map(offer).filter(|o| shared(&o.url) && o.repo_id.len() == 64).collect()
}

fn fetch() -> Result<HttpFetch, Error> {
    HttpFetch::new(Arc::default())
        .map_err(|e| said("android.web_client", "web connections cannot start: %1", &[&e]))
}

pub(crate) fn add(url: &str) -> Result<Value, Error> {
    if !shared(url) {
        return Err(said(
            "android.repo_https",
            "a repository address starts with https://, or http:// for this phone",
            &[],
        ));
    }
    std::fs::create_dir_all(dir()?)?;
    let info = wprs_format::add_repo(&dir()?, &fetch()?, url, now())?;
    store::changed();
    Ok(json!({"repo_id": info.repo_id, "name": info.name, "packages": info.packages}))
}

pub(crate) fn confirm(repo: &str, add_it: bool) -> Result<Value, Error> {
    let repo = repo.trim().to_ascii_lowercase();
    let offers = read(|s| s.offers.clone());
    let matches = |o: &&Offer| o.id == repo || (repo.len() >= 8 && o.repo_id.starts_with(&repo));
    let offer = offers.iter().find(matches).cloned();
    let offer = offer
        .ok_or_else(|| said("android.no_repo_offer", "no repository %1 waits for a decision", &[&repo]))?;
    if !Anchor::parse(&offer.url).is_ok_and(|a| a.repo_id.is_none_or(|id| id == offer.repo_id)) {
        return Err(said(
            "android.repo_other",
            "the synced address names another repository; it was not added",
            &[],
        ));
    }
    let glue = if offer.url.contains('?') { "&" } else { "?" };
    let url = match offer.url.contains("repo_id=") {
        true => offer.url.clone(),
        false => format!("{}{glue}repo_id={}", offer.url, offer.repo_id),
    };
    let added = if add_it { add(&url)? } else { Value::Null };
    let key = format!("repos/{}", offer.id);
    update(|s| {
        s.offers.retain(|o| o.id != offer.id);
        if !add_it {
            s.declined.insert(key);
        }
    })?;
    Ok(added)
}

pub(crate) fn list() -> Result<Value, Error> {
    let repos = wprs_format::repos(&dir()?, now()).unwrap_or_default();
    let repos: Vec<Value> = repos
        .iter()
        .map(|r| {
            json!({"repo_id": r.repo_id, "name": r.name, "url": r.url,
        "packages": r.packages, "stale": r.stale, "fingerprints": r.fingerprints, "broken": r.broken})
        })
        .collect();
    let offers = read(|s| s.offers.clone());
    Ok(json!({"repos": repos, "offers": offers}))
}

pub(crate) fn remove(repo_id: &str) -> Result<Value, Error> {
    wprs_format::remove_repo(&dir()?, repo_id)?;
    store::changed();
    Ok(Value::Null)
}

pub(crate) fn browse(query: &str) -> Result<Value, Error> {
    Ok(listed(wprs_format::browse(&dir()?, query)?, library::hidden(), &library::languages()))
}

pub(crate) fn listed(found: Vec<wprs_format::Item>, hide: bool, languages: &[String]) -> Value {
    let mature = |i: &wprs_format::Item| i.release.content.as_ref().is_some_and(wprs_format::Rated::mature);
    let items = found.iter().filter(|i| !hide || !mature(i)).take(200).map(|i| {
        json!({"repo_id": i.repo_id, "repo": i.repo_name, "id": i.id,
        "version": i.release.version, "title": i.release.title.pick(languages), "kind": i.release.kind,
        "size": i.release.size, "installed": !i.installed.is_empty(), "mature": mature(i)})
    });
    json!(items.collect::<Vec<_>>())
}

pub(crate) fn install(repo_id: &str, id: &str) -> Result<Value, Error> {
    let spec = format!("{repo_id}/{id}");
    let done = wprs_format::install(&dir()?, &fetch()?, &spec, now(), false, false)?;
    let (uid, _) = store::store(&mut std::fs::File::open(&done.path)?, &format!("{id}.wpak"))?;
    drop(std::fs::remove_file(&done.path));
    let mut items = installed();
    items.insert(uid.clone(), (done.repo_id, done.id));
    store::write(&dir()?.join(ITEMS), &serde_json::to_vec(&items)?)?;
    store::changed();
    Ok(json!({"item": uid}))
}
