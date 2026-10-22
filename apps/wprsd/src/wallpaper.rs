use crate::compositor::{Content, Picture, Spec};
use crate::daemon::Daemon;
use anyhow::Context;
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use wprs_core::Fit;

pub(crate) type Upload = ((u32, u32), Fit, PathBuf, Option<SystemTime>);

fn modified(path: &Path) -> anyhow::Result<Option<SystemTime>> {
    let metadata = std::fs::metadata(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(metadata.modified().ok())
}

impl Daemon {
    pub fn configure_output(&self, output: &str) -> Option<Arc<RgbaImage>> {
        let (size, resolved, uploaded) = {
            let st = self.state();
            let info = st.outputs.get(output)?;
            (info.size, st.config.resolve(output), info.uploaded.clone())
        };
        let (content, upload) = match resolved.image {
            None => (Content::Color, None),
            Some(path) if wprs_video::is_video(&path) => (Content::Video(path), None),
            Some(file) => match self.picture(file, (size, resolved.fit), uploaded) {
                Ok((picture, upload)) => (Content::Image(picture), Some(upload)),
                Err(e) => {
                    tracing::warn!(
                        output,
                        "cannot show the image ({}); showing the color instead",
                        e.root_cause()
                    );
                    tracing::debug!("{e:#}");
                    (Content::Color, None)
                }
            },
        };
        let pixels = match &content {
            Content::Image(picture) => picture.pixels.clone(),
            _ => None,
        };
        self.compositor.configure(
            output,
            size,
            Spec { content, fit: resolved.fit, background: resolved.background },
        );
        if let Some(info) = self.state().outputs.get_mut(output) {
            info.uploaded = upload;
        }
        self.apply_activity();
        pixels
    }

    pub fn configure_all(&self) {
        let outputs: Vec<String> = self.state().outputs.keys().cloned().collect();
        let _decoded: Vec<_> = outputs.iter().filter_map(|o| self.configure_output(o)).collect();
    }

    fn picture(
        &self,
        file: PathBuf,
        (size, fit): ((u32, u32), Fit),
        uploaded: Option<Upload>,
    ) -> anyhow::Result<(Picture, Upload)> {
        let modified = modified(&file)?;
        let upload = (size, fit, file.clone(), modified);
        let pixels =
            (uploaded.as_ref() != Some(&upload)).then(|| self.load_image(&file, modified)).transpose()?;
        Ok((Picture { file, modified, pixels }, upload))
    }

    fn load_image(&self, path: &Path, modified: Option<SystemTime>) -> anyhow::Result<Arc<RgbaImage>> {
        let cached =
            self.state().images.get(path).filter(|(m, _)| *m == modified).and_then(|(_, w)| w.upgrade());
        if let Some(image) = cached {
            return Ok(image);
        }
        let image = wprs_render::decode_image(path, self.compositor.max_edge())
            .with_context(|| format!("cannot decode {}", path.display()))?;
        let image = Arc::new(image);
        let mut st = self.state();
        st.images.retain(|_, (_, weak)| weak.strong_count() > 0);
        st.images.insert(path.to_owned(), (modified, Arc::downgrade(&image)));
        Ok(image)
    }

    pub fn set_wallpaper(&self, path: &Path, output: &str, fit: Option<Fit>) -> anyhow::Result<()> {
        let path = std::fs::canonicalize(path).with_context(|| not_found(path))?;
        anyhow::ensure!(path.is_file(), "{} is not a file", path.display());
        let _decoded = match wprs_video::is_video(&path) {
            true => {
                let size =
                    wprs_video::probe(&path).with_context(|| format!("cannot play {}", path.display()))?;
                tracing::debug!(path = %path.display(), ?size, "video accepted");
                None
            }
            false => Some(self.load_image(&path, modified(&path)?)?),
        };
        self.change_output(output, |c| {
            c.set_image(output, path, fit);
            Ok(())
        })
    }
}

fn not_found(path: &Path) -> String {
    match crate::flatpak::app_id() {
        Some(id) => format!(
            "cannot find {} (if it exists, give access to its folder with: flatpak override --user --filesystem=\"{}:ro\" {id})",
            path.display(),
            path.parent().unwrap_or(path).display()
        ),
        None => format!("cannot find {}", path.display()),
    }
}
