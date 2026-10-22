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
    pub fn upload(self: &Arc<Self>, path: &str) -> Result<u64> {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err(invalid("give the absolute path of a .wpak package"));
        }
        let size =
            std::fs::metadata(&path).map_err(|e| invalid(format!("cannot read the package: {e}")))?.len();
        self.linked()?;
        let target = path.display().to_string();
        Ok(self.start_job(JobKind::Upload, &target, move |d, ctl| d.send_package(&path, size, ctl)))
    }

    fn send_package(&self, path: &Path, size: u64, ctl: &Ctl) -> Result<Value> {
        let (api, _) = self.account_api(true)?;
        let sha256 = wprs_format::sha256_file(path)?;
        let opened = block_on(api.post("/api/v1/uploads", json!({"size": size, "sha256": sha256})));
        let id = opened.map_err(sync_error)?["id"].as_str().unwrap_or_default().to_owned();
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
