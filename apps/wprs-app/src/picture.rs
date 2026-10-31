use crate::bus;
use crate::editor::qobject::Editor;
use anyhow::Result;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use image::RgbaImage;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wprs_core::edit::Edit;
use wprs_depth::{Map, Places};
use wprs_render::{RenderError, Renderer, decode_image};

const EDGE: u32 = 1024;
const SOFTWARE_EDGE: u32 = 512;
const THROTTLE: Duration = Duration::from_millis(40);
static DRAWN: AtomicU64 = AtomicU64::new(0);

pub struct Picture {
    renderer: &'static Renderer,
    image: RgbaImage,
    file: PathBuf,
    depth: Mutex<Option<(String, Arc<Map>)>>,
}

pub fn trouble(e: &anyhow::Error) -> &'static str {
    match e.downcast_ref::<RenderError>() {
        Some(RenderError::NoAdapter(_) | RenderError::Device(_)) => "gpu",
        None if e.downcast_ref::<image::ImageError>().is_some() => "decode",
        _ => "draw",
    }
}

impl Picture {
    pub fn open(file: &Path) -> Result<Self> {
        let image = decode_image(file, 2 * EDGE)?;
        let renderer = Renderer::shared()?;
        Ok(Self { renderer, image, file: file.to_owned(), depth: Mutex::default() })
    }

    pub fn lost(&self) -> bool {
        self.renderer.lost()
    }

    fn depth(&self, edit: &Edit) -> Option<Arc<Map>> {
        let (places, depth) = (Places::default(), edit.depth.clone().unwrap_or_default());
        let found = places.find(&self.file, &edit.source.sha256, &depth)?;
        let key =
            format!("{found:?}{:?}{depth:?}", std::fs::metadata(&found).and_then(|m| m.modified()).ok());
        let mut kept = self.depth.lock().unwrap();
        if kept.as_ref().is_none_or(|(k, _)| *k != key) {
            let map = places.map(&self.file, &edit.source.sha256, &depth, 2 * EDGE).ok()??;
            *kept = Some((key, Arc::new(map)));
        }
        kept.as_ref().map(|(_, map)| map.clone())
    }

    pub fn draw(&self, edit: &Edit, whole: bool) -> Result<RgbaImage> {
        let mut edit = Edit { parallax: None, ..edit.clone() };
        if whole {
            edit.geometry.crop = None;
        }
        let depth = edit.needs_depth().then(|| self.depth(&edit)).flatten();
        let (w, h) = edit.geometry.frame(self.image.dimensions());
        let edge = if self.renderer.software() { SOFTWARE_EDGE } else { EDGE };
        let fit = (edge as f32 / w.max(h) as f32).min(1.0);
        let size = |v: u32| ((v as f32 * fit).round() as u32).max(1);
        let developed = self.renderer.develop(&self.image, &edit, depth.as_deref(), (size(w), size(h)))?;
        Ok(self.renderer.read_image(&developed)?)
    }
}

pub fn export(file: &Path, edit: &Edit, out: &Path, quality: u32) -> Result<()> {
    let renderer = Renderer::shared()?;
    let image = decode_image(file, renderer.max_edge())?;
    let depth = edit.depth.clone().unwrap_or_default();
    let map = match edit.needs_depth() {
        true => Places::default().map(file, &edit.source.sha256, &depth, 4096)?,
        false => None,
    };
    let pixels = renderer.export_edit(&image, edit, map.as_ref(), None)?;
    Ok(wprs_render::save(&pixels, out, quality.clamp(1, 100) as u8)?)
}

pub fn write(pixels: &RgbaImage) -> Result<PathBuf> {
    let dir = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    let id = DRAWN.fetch_add(1, Ordering::Relaxed);
    let path = dir.join(format!("wprs-app-{}-{id}.ppm", std::process::id()));
    let mut data = format!("P6\n{} {}\n255\n", pixels.width(), pixels.height()).into_bytes();
    data.extend(pixels.pixels().flat_map(|p| [p[0], p[1], p[2]]));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)?;
    file.write_all(&data)?;
    Ok(path)
}

impl Editor {
    pub fn paint(self: Pin<&mut Self>) {
        let (file, qt) = (self.file.clone(), self.qt_thread());
        bus::spawn(async move {
            let made = tokio::task::spawn_blocking(move || Picture::open(&file)).await;
            qt.queue(move |mut editor| match made.map_err(anyhow::Error::from).and_then(|m| m) {
                Ok(picture) => {
                    editor.as_mut().rust_mut().art = Some(Arc::new(picture));
                    editor.draw();
                }
                Err(e) => editor.fail(&e),
            })
            .ok();
        });
    }

    fn fail(self: Pin<&mut Self>, e: &anyhow::Error) {
        eprintln!("wprs-app: the picture cannot be drawn: {e:#}");
        self.set_trouble(QString::from(trouble(e)));
    }

    pub fn view(mut self: Pin<&mut Self>, whole: bool) {
        self.as_mut().rust_mut().wanted = whole;
        self.draw();
    }

    pub fn refresh(self: Pin<&mut Self>) {
        if self.edit.needs_depth() {
            self.draw();
        }
    }

    pub fn draw(mut self: Pin<&mut Self>) {
        let Some(art) = self.art.clone() else { return };
        if self.drawing {
            self.as_mut().rust_mut().redraw = true;
            return;
        }
        let edit = if self.comparing { self.edit.before() } else { self.edit.clone() };
        let (whole, qt) = (self.wanted, self.qt_thread());
        self.as_mut().rust_mut().drawing = true;
        bus::spawn(async move {
            let started = Instant::now();
            let drawn = tokio::task::spawn_blocking(move || write(&art.draw(&edit, whole)?)).await;
            let drawn = drawn.map_err(anyhow::Error::from).and_then(|d| d);
            tokio::time::sleep(THROTTLE.saturating_sub(started.elapsed())).await;
            let left = drawn.as_ref().ok().cloned();
            let shown = qt.queue(move |mut editor| {
                editor.as_mut().rust_mut().drawing = false;
                match drawn {
                    Ok(path) => {
                        let old = editor.as_mut().rust_mut().drawn.replace(path.clone());
                        editor.as_mut().set_trouble(QString::default());
                        editor.as_mut().set_whole(whole);
                        editor.as_mut().set_picture(QString::from(&bus::url(&path.to_string_lossy())));
                        old.map(std::fs::remove_file);
                    }
                    Err(_) if editor.art.as_ref().is_some_and(|art| art.lost()) => {
                        editor.as_mut().rust_mut().art = None;
                        editor.as_mut().paint();
                    }
                    Err(e) => editor.as_mut().fail(&e),
                }
                if std::mem::take(&mut editor.as_mut().rust_mut().redraw) {
                    editor.draw();
                }
            });
            if shown.is_err() {
                left.map(std::fs::remove_file);
            }
        });
    }
}
