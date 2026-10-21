//! Daemon state: configuration, known outputs, renderer and exporter.

use anyhow::Context;
use image::RgbaImage;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use wprs_core::{Config, Fit, dbus};
use wprs_pipewire::{Exporter, StreamInfo, VideoFrame};
use wprs_render::{Renderer, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct OutputSize {
    pub width: u32,
    pub height: u32,
}

struct CachedImage {
    modified: Option<SystemTime>,
    image: Arc<RgbaImage>,
}

struct State {
    config: Config,
    config_path: PathBuf,
    outputs: HashMap<String, OutputSize>,
    images: HashMap<PathBuf, CachedImage>,
}

pub struct Daemon {
    renderer: Renderer,
    exporter: Exporter,
    state: Mutex<State>,
    /// Serializes renders so concurrent requests do not race on the GPU queue.
    render_lock: Mutex<()>,
}

impl Daemon {
    pub fn new(config: Config, config_path: PathBuf) -> anyhow::Result<Self> {
        let renderer = Renderer::new().context("cannot initialize the GPU renderer")?;
        tracing::info!(adapter = renderer.adapter(), "renderer ready");
        let exporter = Exporter::start().context("cannot connect to PipeWire")?;
        Ok(Self {
            renderer,
            exporter,
            state: Mutex::new(State {
                config,
                config_path,
                outputs: HashMap::new(),
                images: HashMap::new(),
            }),
            render_lock: Mutex::new(()),
        })
    }

    pub fn adapter(&self) -> &str {
        self.renderer.adapter()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Validates a stream request and normalizes the output name.
    pub fn check_request(&self, output: &str, width: u32, height: u32) -> Result<String, String> {
        let max = self.renderer.max_edge().min(dbus::MAX_STREAM_EDGE);
        if width == 0 || height == 0 || width > max || height > max {
            return Err(format!("invalid size {width}x{height} (allowed 1..={max})"));
        }
        let output = output.trim();
        if output.len() > 128 || output.chars().any(|c| c.is_control()) {
            return Err("invalid output name".into());
        }
        Ok(if output.is_empty() {
            dbus::DEFAULT_OUTPUT.to_owned()
        } else {
            output.to_owned()
        })
    }

    /// Creates or reuses the stream for `output` and reports it through `reply`.
    pub fn ensure_stream(
        &self,
        output: &str,
        width: u32,
        height: u32,
        reply: Box<dyn FnOnce(Result<StreamInfo, String>) + Send>,
    ) {
        self.state()
            .outputs
            .insert(output.to_owned(), OutputSize { width, height });
        self.exporter.ensure_stream(output, width, height, reply);
    }

    /// Renders `output` with its current settings and pushes the frame.
    pub fn render_output(&self, output: &str) -> anyhow::Result<()> {
        let (size, resolved) = {
            let st = self.state();
            let Some(size) = st.outputs.get(output).copied() else {
                return Ok(());
            };
            (size, st.config.resolve(output))
        };
        let image = match resolved.image.as_deref() {
            Some(path) => match self.load_image(path) {
                Ok(img) => Some(img),
                Err(e) => {
                    tracing::warn!("{e:#}; showing the background color instead");
                    None
                }
            },
            None => None,
        };
        let frame = {
            let _guard = self.render_lock.lock().unwrap_or_else(|e| e.into_inner());
            self.renderer.render(
                Scene {
                    image: image.as_deref(),
                    fit: resolved.fit,
                    background: resolved.background,
                },
                size.width,
                size.height,
            )?
        };
        tracing::debug!(
            output,
            width = frame.width,
            height = frame.height,
            "frame rendered"
        );
        self.exporter.push_frame(
            output,
            Arc::new(VideoFrame {
                width: frame.width,
                height: frame.height,
                stride: frame.stride,
                data: frame.data,
            }),
        )?;
        Ok(())
    }

    /// Renders every output that has a stream.
    pub fn render_all(&self) {
        let outputs: Vec<String> = self.state().outputs.keys().cloned().collect();
        for output in outputs {
            if let Err(e) = self.render_output(&output) {
                tracing::error!(output, "render failed: {e:#}");
            }
        }
    }

    /// Decodes an image, reusing the cached copy while the file is unchanged.
    pub fn load_image(&self, path: &Path) -> anyhow::Result<Arc<RgbaImage>> {
        let modified = std::fs::metadata(path)
            .with_context(|| format!("cannot read {}", path.display()))?
            .modified()
            .ok();
        if let Some(c) = self.state().images.get(path)
            && c.modified == modified
        {
            return Ok(c.image.clone());
        }
        let image = image::ImageReader::open(path)
            .with_context(|| format!("cannot open {}", path.display()))?
            .with_guessed_format()
            .with_context(|| format!("cannot read {}", path.display()))?
            .decode()
            .with_context(|| format!("cannot decode {}", path.display()))?
            .into_rgba8();
        let image = Arc::new(image);
        let mut st = self.state();
        // Keep the cache small: only images that are currently configured.
        let in_use: Vec<PathBuf> = std::iter::once(&st.config.default)
            .chain(st.config.outputs.values())
            .filter_map(|o| o.image.clone())
            .collect();
        st.images.retain(|p, _| in_use.contains(p));
        st.images.insert(
            path.to_owned(),
            CachedImage {
                modified,
                image: image.clone(),
            },
        );
        Ok(image)
    }

    /// Sets and persists the wallpaper, then re-renders affected outputs.
    pub fn set_wallpaper(&self, path: PathBuf, output: &str, fit: Option<Fit>) -> anyhow::Result<()> {
        let path = std::fs::canonicalize(&path).with_context(|| format!("cannot find {}", path.display()))?;
        self.load_image(&path)?;
        {
            let mut st = self.state();
            st.config.set_image(output, path, fit);
            let config_path = st.config_path.clone();
            st.config.save(&config_path)?;
        }
        if output.is_empty() {
            self.render_all();
        } else {
            self.render_output(output)?;
        }
        Ok(())
    }

    /// Re-reads the configuration file and re-renders everything.
    pub fn reload(&self) -> anyhow::Result<()> {
        {
            let mut st = self.state();
            let config = Config::load(&st.config_path)?;
            st.config = config;
            st.images.clear();
        }
        self.render_all();
        Ok(())
    }

    /// JSON description of the daemon state, for diagnostics.
    pub fn state_json(&self) -> String {
        let st = self.state();
        let outputs: serde_json::Map<String, serde_json::Value> = st
            .outputs
            .iter()
            .map(|(name, size)| {
                let r = st.config.resolve(name);
                (
                    name.clone(),
                    serde_json::json!({
                        "width": size.width,
                        "height": size.height,
                        "image": r.image,
                        "fit": r.fit.as_str(),
                    }),
                )
            })
            .collect();
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "adapter": self.renderer.adapter(),
            "config": st.config_path,
            "outputs": outputs,
        })
        .to_string()
    }
}
