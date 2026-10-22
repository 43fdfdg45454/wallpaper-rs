use super::views::{Kept, Local};
use crate::daemon::Daemon;
use crate::error::{Result, invalid, not_found};
use crate::jobs::now;
use crate::kinds::{local, parse_url};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::Arc;
use wprs_core::model::RepoOffer;
use wprs_format::Anchor;
use wprs_sync::{Ids, Replica, View};

fn shared(url: &str) -> bool {
    parse_url(url, true).is_ok_and(|u| u.scheme() == "https" || u.scheme() == "http" && local(&u))
}

fn field(view: &View, name: &str) -> String {
    view.get(name).and_then(Value::as_str).unwrap_or_default().to_owned()
}

impl Daemon {
    pub fn repo_views(
        &self,
        ids: &Ids,
        replica: &Replica,
        present: &BTreeSet<String>,
        local: &mut Local,
        kept: &mut Kept,
    ) -> Result<()> {
        for repo in wprs_format::repos(&self.store.data, now())?.into_iter().filter(|r| shared(&r.url)) {
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
        let others =
            replica.bases.iter().filter(|(key, _)| key.starts_with("repos/") && !present.contains(*key));
        let others: Vec<(String, View)> = others
            .filter(|(key, _)| !local.views.contains_key(*key))
            .filter_map(|(key, (_, doc))| Some((key.clone(), doc.view()?)))
            .collect();
        local.views.extend(others);
        Ok(())
    }

    pub fn offers(&self, local: &Local, replica: &Replica) -> Vec<RepoOffer> {
        let declined = self.saved().declined.clone();
        let waiting = replica.bases.iter().filter(|(key, _)| key.starts_with("repos/"));
        let waiting = waiting.filter(|(key, _)| !local.repos.contains(*key) && !declined.contains(*key));
        let offer = |(key, view): (&String, View)| RepoOffer {
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

    pub fn apply_repo(self: &Arc<Self>, local: &Local, key: &str, view: Option<View>) -> Result<()> {
        if view.is_some() || !local.repos.contains(key) {
            return Ok(());
        }
        let id = local.views.get(key).map(|v| field(v, "repo_id")).unwrap_or_default();
        tracing::info!("a repository removed on another device is removed here, keeping its packages");
        self.remove_repo(&id, false)
    }

    pub fn confirm_repo(self: &Arc<Self>, repo: &str, add: bool) -> Result<u64> {
        self.linked()?;
        let repo = repo.trim().to_ascii_lowercase();
        let offers = self.saved().offers.clone();
        let matches = |o: &&RepoOffer| o.id == repo || (repo.len() >= 8 && o.repo_id.starts_with(&repo));
        let offer = offers.iter().find(matches).cloned();
        let offer = offer.ok_or_else(|| not_found(format!("no repository {repo} waits for a decision")))?;
        let anchored =
            Anchor::parse(&offer.url).is_ok_and(|a| a.repo_id.is_none_or(|id| id == offer.repo_id));
        if !anchored {
            return Err(invalid("the synced address names another repository; it was not added"));
        }
        let url = match offer.url.contains("repo_id=") {
            true => offer.url.clone(),
            false => format!(
                "{}{}repo_id={}",
                offer.url,
                if offer.url.contains('?') { "&" } else { "?" },
                offer.repo_id
            ),
        };
        let key = format!("repos/{}", offer.id);
        self.update(|s| {
            s.offers.retain(|o| o.id != offer.id);
            if !add {
                s.declined.insert(key.clone());
            }
        })?;
        if add { self.add_repo(&url) } else { Ok(0) }
    }
}
