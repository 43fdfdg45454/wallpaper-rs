use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, failed, invalid};
use crate::inputs::{Grants, Hub, View, hub};
use crate::items::reply;
use crate::streams::OutputInfo;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use wprs_core::model::{Input, InputsReply, MODEL};

pub struct Inputs(pub Weak<Daemon>);

fn grants_path(daemon: &Daemon) -> PathBuf {
    daemon.store.data.join("inputs.json")
}

pub(crate) fn load(path: &Path) -> Grants {
    let Ok(bytes) = std::fs::read(path) else { return Grants::new() };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        tracing::warn!("ignoring unreadable input permissions: {e}");
        Grants::new()
    })
}

pub(crate) fn save(path: &Path, value: &impl serde::Serialize) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    crate::store::private_dir(dir)?;
    let mut file = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.persist(path).map(drop).map_err(|e| e.error)
}

pub(crate) fn scene_id(scene: &str) -> Result<&str> {
    let valid = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c);
    let fine = (1..=255).contains(&scene.len()) && scene.chars().all(valid);
    fine.then_some(scene).ok_or_else(|| invalid(format!("{scene} is not a scene package id")))
}

pub(crate) fn start(daemon: &Arc<Daemon>, connection: zbus::Connection) {
    crate::inputs::start(daemon, connection, load(&grants_path(daemon)));
}

impl Daemon {
    pub(crate) fn views(&self) -> BTreeMap<String, View> {
        let st = self.state();
        let view = |info: &OutputInfo| View {
            active: st.active(info) && !st.locked,
            covered: info.covered(),
            rect: info.screen.as_ref().map(|s| [s.x.into(), s.y.into(), s.width.into(), s.height.into()]),
        };
        st.outputs.iter().map(|(name, info)| (name.clone(), view(info))).collect()
    }
}

impl Inputs {
    fn parts(&self) -> Result<(Arc<Daemon>, &'static Arc<Hub>)> {
        let stopping = || failed("the daemon is stopping");
        Ok((self.0.upgrade().ok_or_else(stopping)?, hub().ok_or_else(stopping)?))
    }

    async fn set(&self, scene: String, input: String, granted: bool) -> Result<()> {
        let (d, hub) = self.parts()?;
        let input =
            Input::parse(&input).ok_or_else(|| invalid(format!("{input} is not audio or pointer")))?;
        let scene = scene_id(&scene)?.to_owned();
        d.run(move |d| hub.set_grant(&grants_path(d), &scene, input, granted)).await
    }
}

#[zbus::interface(name = "io.github.wallpaper_rs.Inputs1")]
impl Inputs {
    async fn grant(&self, scene: String, input: String) -> Result<()> {
        self.set(scene, input, true).await
    }

    async fn revoke(&self, scene: String, input: String) -> Result<()> {
        self.set(scene, input, false).await
    }

    #[zbus(out_args("json"))]
    async fn get_inputs(&self) -> Result<String> {
        let (d, hub) = self.parts()?;
        let (serial, tracking) = (d.changes.serial(), *hub.tracking.borrow());
        let control = lock(&hub.control);
        let (grants, shown, capturing) = (control.grants.clone(), control.shown.clone(), control.capturing);
        let parallax = control.parallax.clone();
        reply(InputsReply { model: MODEL, serial, capturing, tracking, grants, shown, parallax })
    }

    async fn set_pointer(&self, x: i32, y: i32) -> Result<()> {
        self.parts()?.1.set_pointer(x.into(), y.into());
        Ok(())
    }
}
