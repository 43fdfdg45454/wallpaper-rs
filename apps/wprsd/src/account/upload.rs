use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid};
use crate::jobs::Ctl;
use reqwest::Method;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::model::{JobKind, UploadOutput};

impl Daemon {
    pub fn upload(self: &Arc<Self>, path: &str, visibility: &str) -> Result<u64> {
        if !matches!(visibility, "public" | "unlisted") {
            return Err(invalid("the visibility is public or unlisted"));
        }
        let (path, visibility) = (PathBuf::from(path), visibility.to_owned());
        if !path.is_absolute() {
            return Err(invalid("give the absolute path of a .wpak package"));
        }
        let size =
            std::fs::metadata(&path).map_err(|e| invalid(format!("cannot read the package: {e}")))?.len();
        self.linked()?;
        let target = wprs_sync::hex(&wprs_sync::random::<8>());
        Ok(self
            .start_job(JobKind::Upload, &target, move |d, ctl| d.send_package(&path, size, &visibility, ctl)))
    }

    pub fn own_packages(&self) -> Result<String> {
        let (api, _) = self.account_api(true)?;
        let found = block_on(api.get("/api/v1/me/packages")).map_err(sync_error)?;
        Ok(json!({"items": found["items"]}).to_string())
    }

    pub fn set_visibility(&self, id: &str, version: &str, visibility: &str) -> Result<()> {
        if !wprs_format::valid_id(id) || !matches!(visibility, "public" | "unlisted") {
            return Err(invalid("give a package id and public or unlisted"));
        }
        let (api, _) = self.account_api(true)?;
        let version = Some(version).filter(|v| !v.is_empty());
        let body = json!({"visibility": visibility, "version": version});
        block_on(api.post(&format!("/api/v1/me/packages/{id}/visibility"), body)).map_err(sync_error)?;
        Ok(())
    }

    fn send_package(&self, path: &Path, size: u64, visibility: &str, ctl: &Ctl) -> Result<Value> {
        let (api, _) = self.account_api(true)?;
        let sha256 = wprs_format::sha256_file(path)?;
        let asked = json!({"size": size, "sha256": sha256, "visibility": visibility});
        let opened = block_on(api.post("/api/v1/uploads", asked)).map_err(sync_error)?;
        let id = opened["id"].as_str().unwrap_or_default().to_owned();
        if visibility != "public" && opened["visibility"] != visibility {
            drop(block_on(api.slow(Method::DELETE, &format!("/api/v1/uploads/{id}"))));
            return Err(failed("this server does not offer unlisted uploads; nothing was uploaded"));
        }
        ctl.check()?;
        block_on(api.put_file(&format!("/api/v1/uploads/{id}"), path, size)).map_err(sync_error)?;
        ctl.check()?;
        let status = block_on(api.slow(Method::POST, &format!("/api/v1/uploads/{id}/finalize")))
            .map_err(sync_error)?;
        let text = |field: &str| status[field].as_str().map(String::from);
        let errors: Vec<String> = status["errors"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| e.as_str())
            .map(String::from)
            .collect();
        let state = text("state").unwrap_or_default();
        if matches!(state.as_str(), "failed" | "rejected") {
            return Err(failed(format!("the server refused the package: {}", errors.join("; "))));
        }
        let output = UploadOutput { id, state, package: text("package"), version: text("version"), errors };
        Ok(serde_json::to_value(output).unwrap_or_default())
    }
}
