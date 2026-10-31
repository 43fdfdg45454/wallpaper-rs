use crate::daemon::Daemon;
use crate::error::Result;
use crate::items::reply;
use crate::repos::newer;
use std::collections::HashMap;
use wprs_core::model::{InstalledPackage, InstalledReply, MODEL};
use wprs_format::Installed;

impl Daemon {
    pub fn latest(&self) -> Result<HashMap<(String, String), (String, String)>> {
        let langs = wprs_core::locale::languages();
        let notes = |r: &wprs_format::Release| r.changes.as_ref().map(|c| c.pick(&langs)).unwrap_or_default();
        let items = wprs_format::browse(&self.store.data, "")?.into_iter();
        Ok(items.map(|i| ((i.repo_id, i.id), (notes(&i.release), i.release.version))).collect())
    }

    pub fn list_installed(&self) -> Result<String> {
        let serial = self.changes.serial();
        let (list, latest) = (wprs_format::installed(&self.store.data)?, self.latest()?);
        let model = |record: &Installed| {
            let newest =
                list.iter().filter(|i| i.id == record.id).all(|i| !newer(&i.version, &record.version));
            let available = latest.get(&(record.repo_id.clone(), record.id.clone()));
            let update = available.filter(|(_, v)| newest && newer(v, &record.version));
            let (changes, update) = update.cloned().map_or((String::new(), None), |(c, v)| (c, Some(v)));
            InstalledPackage { changes, ..self.package_model(record, update) }
        };
        reply(InstalledReply { model: MODEL, serial, packages: list.iter().map(model).collect() })
    }

    pub fn package_model(&self, record: &Installed, update: Option<String>) -> InstalledPackage {
        let item = record.path.to_str().and_then(|p| self.store.read(|lib| lib.resolve_item(p)).ok());
        let revoked = record.revoked.and_then(|r| serde_json::from_value(serde_json::to_value(r).ok()?).ok());
        InstalledPackage {
            repo_id: record.repo_id.clone(),
            id: record.id.clone(),
            version: record.version.clone(),
            sha256: record.sha256.clone(),
            size: record.size,
            signer: record.signer.clone(),
            revoked,
            item,
            update,
            changes: String::new(),
        }
    }
}
