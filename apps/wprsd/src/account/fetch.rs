use super::transfer::{BATCH, blob_path};
use super::{block_on, sync_error};
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid, not_found};
use crate::jobs::{Ctl, now};
use crate::store::private_dir;
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::model::JobKind;
use wprs_sync::{SEALED, SEGMENT, open_segment, sealed_size};

fn place(dir: &Path, name: &str, part: &Path) -> Result<PathBuf> {
    let (stem, ext) = name.rsplit_once('.').filter(|(stem, _)| !stem.is_empty()).unwrap_or((name, ""));
    for n in 1..10_000 {
        let named = match (n, ext) {
            (1, _) => name.to_owned(),
            (n, "") => format!("{stem} ({n})"),
            (n, ext) => format!("{stem} ({n}).{ext}"),
        };
        match fs::hard_link(part, dir.join(&named)) {
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            linked => {
                linked?;
                fs::remove_file(part)?;
                return Ok(dir.join(named));
            }
        }
    }
    Err(failed("no free file name"))
}

impl Daemon {
    pub fn fetch_file(self: &Arc<Self>, id: &str) -> Result<u64> {
        self.require_ready()?;
        let blob = self.remote_file(id.trim()).ok_or_else(|| not_found(format!("no private file {id}")))?;
        let name = blob["name"].as_str().unwrap_or_default();
        if wprs_library::Kind::detect(Path::new(name)).is_none() {
            return Err(invalid(format!("this computer cannot show {name}")));
        }
        Ok(self.start_job(JobKind::Upload, "fetch", move |d, ctl| d.download(&blob, ctl)))
    }

    fn download(self: &Arc<Self>, blob: &Value, ctl: &Ctl) -> Result<Value> {
        let (api, secrets) = self.account_api(true)?;
        let ids = secrets.sync_key()?.ids();
        let (hash, size) =
            (blob["hash"].as_str().unwrap_or_default(), blob["size"].as_u64().unwrap_or_default());
        let id = ids.stored(hash.as_bytes());
        if blob["id"] != id.as_str() || hash.len() != 64 {
            return Err(failed("the record of this private file is broken"));
        }
        let name: String = blob["name"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_control() && *c != '/')
            .collect();
        let name =
            Some(name.trim_start_matches('.').to_owned()).filter(|n| !n.is_empty()).unwrap_or("file".into());
        let (dir, key) = (self.store.files(), ids.file_key(hash.as_bytes()));
        private_dir(&dir)?;
        let part = dir.join(format!(".part-{}", &id[..16]));
        let (segments, total) = (size.div_ceil(SEGMENT).max(1), sealed_size(size));
        let mut file = OpenOptions::new().create(true).write(true).truncate(false).mode(0o600).open(&part)?;
        let mut index = (file.metadata()?.len() / SEGMENT).min(segments);
        file.set_len(index * SEGMENT)?;
        file.seek(SeekFrom::End(0))?;
        while index < segments {
            ctl.check()?;
            let offset = index * SEALED;
            let length = (BATCH * SEALED).min(total - offset);
            let path = format!("{}&length={length}", blob_path(&id, offset));
            let bytes = block_on(api.get_bytes(&path, length as usize)).map_err(sync_error)?;
            if bytes.len() as u64 != length {
                return Err(failed("the server sent a short piece of the file; fetch again"));
            }
            for chunk in bytes.chunks(SEALED as usize) {
                file.write_all(
                    &open_segment(&key, index, index + 1 == segments, chunk).map_err(sync_error)?,
                )?;
                index += 1;
            }
            ctl.progress(index, segments);
        }
        drop(file);
        if wprs_format::sha256_file(&part)? != hash {
            fs::remove_file(&part)?;
            return Err(failed("the downloaded file does not match its hash"));
        }
        let dest = place(&dir, &name, &part)?;
        let uid = self.store.write(|lib| lib.import_file(&dest, now()))?;
        self.library_changed(vec![uid.clone()]);
        self.analyze_job();
        self.account.wake.notify_one();
        Ok(json!({"item": uid}))
    }
}
