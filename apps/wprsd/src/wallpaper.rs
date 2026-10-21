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
        let decode =
            || image::ImageReader::open(path)?.with_guessed_format()?.decode().map_err(anyhow::Error::from);
        let image =
            Arc::new(decode().with_context(|| format!("cannot decode {}", path.display()))?.into_rgba8());
        let mut st = self.state();
        let in_use: Vec<PathBuf> = st.config.images().cloned().collect();
        st.images.retain(|p, _| in_use.contains(p));
        st.images.insert(path.to_owned(), (modified, image.clone()));
        Ok(image)
    }

    pub fn set_wallpaper(&self, path: PathBuf, output: &str, fit: Option<Fit>) -> anyhow::Result<()> {
        let path = std::fs::canonicalize(&path).with_context(|| format!("cannot find {}", path.display()))?;
        anyhow::ensure!(path.is_file(), "{} is not a file", path.display());
        if !wprs_video::is_video(&path) {
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
