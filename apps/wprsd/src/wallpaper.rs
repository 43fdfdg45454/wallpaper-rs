use crate::compositor::{Content, Spec};
use crate::daemon::Daemon;
use anyhow::Context;
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::Fit;

impl Daemon {
    pub fn configure_output(&self, output: &str) {
        let Some((size, resolved)) = ({
            let st = self.state();
            st.outputs.get(output).map(|o| (o.size, st.config.resolve(output)))
        }) else {
            return;
        };
        let content = match resolved.image {
            None => Content::Color,
            Some(path) if wprs_video::is_video(&path) => Content::Video(path),
            Some(path) => self.load_image(&path).map(Content::Image).unwrap_or_else(|e| {
                tracing::warn!("{e:#}; showing the background color instead");
                Content::Color
            }),
        };
        self.compositor.configure(
            output,
            size,
            Spec { content, fit: resolved.fit, background: resolved.background },
        );
        self.apply_activity();
    }

    pub fn configure_all(&self) {
        let outputs: Vec<String> = self.state().outputs.keys().cloned().collect();
        outputs.iter().for_each(|o| self.configure_output(o));
    }

    pub(crate) fn load_image(&self, path: &Path) -> anyhow::Result<Arc<RgbaImage>> {
        let modified = std::fs::metadata(path)
            .with_context(|| format!("cannot read {}", path.display()))?
            .modified()
            .ok();
        if let Some((_, image)) = self.state().images.get(path).filter(|(m, _)| *m == modified) {
            return Ok(image.clone());
        }
        let image = wprs_render::decode_image(path, self.compositor.max_edge())
            .with_context(|| format!("cannot decode {}", path.display()))?;
        let image = Arc::new(image);
        let mut st = self.state();
        let in_use: Vec<PathBuf> = st.config.images().cloned().collect();
        st.images.retain(|p, _| in_use.contains(p));
        st.images.insert(path.to_owned(), (modified, image.clone()));
        Ok(image)
    }

    pub fn set_wallpaper(&self, path: PathBuf, output: &str, fit: Option<Fit>) -> anyhow::Result<()> {
        let remote = path.to_str().is_some_and(|p| p.contains("://"));
        anyhow::ensure!(!remote, "{} is not a local file", path.display());
        let path = std::fs::canonicalize(&path).with_context(|| not_found(&path))?;
        anyhow::ensure!(path.is_file(), "{} is not a file", path.display());
        if wprs_video::is_video(&path) {
            let (width, height) =
                wprs_video::probe(&path).with_context(|| format!("cannot play {}", path.display()))?;
            tracing::info!(path = %path.display(), width, height, "video accepted");
        } else {
            self.load_image(&path)?;
        }
        self.update_config(|c| {
            c.set_image(output, path, fit);
            Ok(())
        })?;
        match output.is_empty() {
            true => self.configure_all(),
            false => self.configure_output(output),
        }
        Ok(())
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
