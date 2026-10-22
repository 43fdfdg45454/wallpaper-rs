use crate::daemon::Daemon;
use crate::error::{Result, invalid};
use crate::jobs::{Ctl, now};
use crate::scan::local;
use crate::store::private_dir;
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::model::{ImportReport, Imported, JobKind, PathError};
use wprs_library::Kind;

impl Daemon {
    pub fn import(self: &Arc<Self>, paths: Vec<String>, copy: bool) -> Result<u64> {
        if paths.is_empty() || paths.len() > 1000 {
            return Err(invalid("give 1 to 1000 files"));
        }
        paths.iter().try_for_each(|path| local(path).map(drop))?;
        Ok(self.start_job(JobKind::Import, "", move |d, ctl| d.import_files(&paths, copy, ctl)))
    }

    fn import_files(self: &Arc<Self>, paths: &[String], copy: bool, ctl: &Ctl) -> Result<Value> {
        let mut report = ImportReport { imported: Vec::new(), failed: Vec::new() };
        for (done, path) in (1..).zip(paths) {
            if ctl.check().is_err() {
                break;
            }
            match self.import_one(Path::new(path), copy) {
                Ok(uid) => report.imported.push(Imported { path: path.into(), uid }),
                Err(e) => report.failed.push(PathError { path: path.into(), error: e.to_string() }),
            }
            ctl.progress(done, paths.len() as u64);
        }
        if !report.imported.is_empty() {
            self.library_changed(report.imported.iter().map(|i| i.uid.clone()).collect());
            self.analyze_job();
        }
        Ok(serde_json::to_value(report).unwrap_or_default())
    }

    fn import_one(&self, path: &Path, copy: bool) -> Result<String> {
        if !fs::metadata(path)?.is_file() || Kind::detect(path).is_none() {
            return Err(invalid("not an image, video or package file"));
        }
        if !copy {
            return self.store.write(|lib| lib.import_file(path, now()));
        }
        let copied = self.copy_in(path)?;
        let titled = |lib: &mut wprs_library::Library| {
            let uid = lib.import_file(&copied, now())?;
            let (title, stem) = (lib.item(&uid)?.title, copied.file_stem().map(|s| s.to_string_lossy()));
            if let Some(original) = path.file_stem().filter(|_| stem.as_deref() == Some(title.as_str())) {
                lib.set_title(&uid, &original.to_string_lossy()).ok();
            }
            Ok::<_, wprs_library::Error>(uid)
        };
        self.store.write(titled).inspect_err(|_| drop(fs::remove_file(&copied)))
    }

    fn copy_in(&self, path: &Path) -> Result<PathBuf> {
        let dir = self.store.files();
        private_dir(&dir)?;
        let name = |n: u32| match (n, path.file_stem(), path.extension()) {
            (1, ..) | (_, None, _) | (_, _, None) => path.file_name().map(PathBuf::from).unwrap_or_default(),
            (n, Some(stem), Some(ext)) => {
                format!("{} ({n}).{}", stem.to_string_lossy(), ext.to_string_lossy()).into()
            }
        };
        for n in 1..10_000 {
            let dest = dir.join(name(n));
            match OpenOptions::new().write(true).create_new(true).mode(0o600).open(&dest) {
                Ok(mut file) => {
                    let copied =
                        File::open(path).and_then(|mut source| std::io::copy(&mut source, &mut file));
                    return copied
                        .map(|_| dest.clone())
                        .inspect_err(|_| drop(fs::remove_file(&dest)))
                        .map_err(Into::into);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(invalid("too many files with this name in the imported-files folder"))
    }
}
