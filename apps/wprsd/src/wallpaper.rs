use crate::apply::Media;
use crate::compositor::{Content, Movie, Picture, Reel, SceneSpec, Spec, lock};
use crate::daemon::Daemon;
use anyhow::Context;
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use wprs_core::{Edit, Fit, Place};
use zbus::fdo::Error::InvalidArgs;

pub(crate) type Upload = ((u32, u32), Fit, PathBuf, Option<SystemTime>, Option<Arc<Edit>>, Place);

static ORDER: Mutex<()> = Mutex::new(());

fn modified(path: &Path) -> anyhow::Result<Option<SystemTime>> {
    let metadata = std::fs::metadata(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(metadata.modified().ok())
}

impl Daemon {
    pub fn configure_output(&self, output: &str) -> Option<Arc<RgbaImage>> {
        let _order = lock(&ORDER);
        let (size, mut resolved, chosen, uploaded, refresh) = {
            let st = self.state();
            let info = st.outputs.get(output)?;
            let (resolved, chosen) = (st.config.resolve(output), st.config.chosen(output));
            (info.size, resolved, chosen, info.uploaded.clone(), st.refresh(output))
        };
        if self.show_pattern(output, size, refresh) {
            return None;
        }
        let (image, mut transition) =
            self.playlist_media(output, resolved.playlist.take()).unwrap_or((resolved.image.take(), None));
        let shown = image.as_deref().map(|p| crate::look::look(chosen, || crate::look::author(p)));
        (resolved.fit, resolved.background) = shown.unwrap_or((resolved.fit, resolved.background));
        let draft = self.state().drafts.get(output).map(|d| (d.pack.clone(), d.hold));
        if draft.is_some() {
            transition = Some(Duration::ZERO);
        }
        let hold = draft.as_ref().and_then(|(_, hold)| *hold);
        let media = match draft {
            Some((pack, _)) => Some(Ok(Media::Scene(pack))),
            None => image.clone().map(|image| self.media(image)),
        };
        let (mut shown_scene, mut shown_video, mut turn, mut fit) = (None, None, None, resolved.fit);
        let (content, upload) = match media {
            None => (Content::Color, None),
            Some(Err(e)) => {
                tracing::warn!(output, "cannot show the package; showing the color instead");
                tracing::debug!("{e:#}");
                (Content::Color, None)
            }
            Some(Ok(Media::Video(path, props))) => {
                let movie = match &props {
                    Some(p) => p.movie(path, &p.values(&self.state().config.scene_values(output, &p.id))),
                    None => {
                        Movie { reel: Reel::plain(path), volume: 100, zoom: 1.0, pan: [0.0; 2], max_fps: 0 }
                    }
                };
                shown_video = props;
                (Content::Video(movie), None)
            }
            Some(Ok(Media::Scene(pack))) => {
                let (values, stale) = {
                    let st = self.state();
                    let stored = st.config.scene_values(output, &pack.props.id);
                    (pack.props.values(&stored), pack.props.stale(&stored))
                };
                stale.iter().for_each(|w| tracing::warn!(output, scene = pack.props.id, "{w}"));
                let (extra, clips) = self.scene_extra(&pack, &values);
                turn = crate::picks::next_turn(&pack.props.properties, &values);
                shown_scene = Some(pack.clone());
                let moving = self.store.read(|lib| lib.moves(&pack.props.id)).unwrap_or(false);
                (Content::Scene(SceneSpec { pack, values, extra, clips, hold, moving }), None)
            }
            Some(Ok(Media::Web(pack))) => (Content::Web(self.page_on(output, pack, &mut shown_video)), None),
            Some(Ok(Media::Picture(file))) => {
                match self.picture(output, file, (size, resolved.fit), uploaded) {
                    Ok((picture, upload)) => {
                        fit = upload.1;
                        (Content::Image(picture), Some(upload))
                    }
                    Err(e) => {
                        tracing::warn!(
                            output,
                            "cannot show the image ({}); showing the color instead",
                            e.root_cause()
                        );
                        tracing::debug!("{e:#}");
                        (Content::Color, None)
                    }
                }
            }
        };
        let (pixels, accent) =
            (content.pixels(), self.accent(image.as_deref(), &content, shown_video.as_deref()));
        let spec = Spec { content, fit, background: resolved.background, transition, refresh };
        self.compositor.configure(output, size, spec);
        let mut st = self.state();
        if let Some(info) = st.outputs.get_mut(output) {
            (info.uploaded, info.scene, info.video, info.accent) = (upload, shown_scene, shown_video, accent);
        }
        st.turns.remove(output);
        st.missed.remove(output);
        st.turns.extend(turn.map(|at| (output.to_owned(), at)));
        drop(st);
        self.turned.notify_one();
        self.apply_activity();
        pixels
    }

    pub fn configure_all(&self) {
        let outputs: Vec<String> = self.state().outputs.keys().cloned().collect();
        let _decoded: Vec<_> = outputs.iter().filter_map(|o| self.configure_output(o)).collect();
    }

    fn picture(
        &self,
        output: &str,
        file: PathBuf,
        (size, fit): ((u32, u32), Fit),
        uploaded: Option<Upload>,
    ) -> anyhow::Result<(Picture, Upload)> {
        let modified = modified(&file)?;
        let edit = self.edit_for(&file);
        let fit = edit.as_ref().and_then(|e| e.fit).filter(|_| fit != Fit::Span).unwrap_or(fit);
        let place = self.place(output, fit, edit.as_deref());
        let depth = edit.as_deref().filter(|e| e.needs_depth()).and_then(|e| self.depth_for(&file, e));
        let upload = (size, fit, file.clone(), modified, edit.clone(), place.clone());
        let edge = || {
            wprs_render::needed_edge(&file, edit.as_deref(), &place, (fit, size), self.compositor.max_edge())
        };
        let pixels = (uploaded.as_ref() != Some(&upload))
            .then(|| self.load_image(&file, modified, Some(edge())))
            .transpose()?;
        pixels.iter().for_each(|image| self.keep_decoded(&file, image));
        Ok((Picture { file, modified, pixels, edit, place, depth }, upload))
    }

    pub(crate) fn load_image(
        &self,
        path: &Path,
        modified: Option<SystemTime>,
        edge: Option<u32>,
    ) -> anyhow::Result<Arc<RgbaImage>> {
        let cached =
            self.state().images.get(path).filter(|(m, _)| *m == modified).and_then(|(_, w)| w.upgrade());
        if let Some(image) = cached.filter(|i| edge.is_none_or(|e| i.width().max(i.height()) + 2 >= e)) {
            return Ok(image);
        }
        let max_edge = self.compositor.max_edge();
        let image = wprs_render::decode_image(path, edge.map_or(max_edge, |e| e.min(max_edge)))
            .with_context(|| format!("cannot decode {}", path.display()))?;
        tracing::debug!("decoded a picture");
        let image = Arc::new(image);
        let mut st = self.state();
        st.images.retain(|_, (_, weak)| weak.strong_count() > 0);
        st.images.insert(path.to_owned(), (modified, Arc::downgrade(&image)));
        Ok(image)
    }

    pub fn set_wallpaper(&self, path: &Path, output: &str, fit: Option<Fit>) -> anyhow::Result<()> {
        let path = std::fs::canonicalize(path).with_context(|| not_found(path))?;
        if wprs_library::Kind::detect(&path) == Some(wprs_library::Kind::Package) {
            return Err(InvalidArgs("import the package and use Library1.Apply".into()).into());
        }
        let _decoded = self.check_media(&path)?;
        self.change_output(output, |c| {
            c.set_image(output, path, fit);
            Ok(())
        })
    }

    pub fn check_media(&self, path: &Path) -> anyhow::Result<Option<Arc<RgbaImage>>> {
        anyhow::ensure!(path.is_file(), "{} is not a file", path.display());
        let file = match self.media(path.to_owned())? {
            Media::Picture(file) => return Ok(Some(self.load_image(&file, modified(&file)?, None)?)),
            Media::Scene(_) => return Ok(None),
            Media::Web(_) => return crate::pages::available().map(|_| None).map_err(|e| anyhow::anyhow!(e)),
            Media::Video(file, _) => file,
        };
        let size = wprs_video::probe(&file).with_context(|| format!("cannot play {}", path.display()))?;
        tracing::debug!(path = %path.display(), ?size, "video accepted");
        Ok(None)
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
