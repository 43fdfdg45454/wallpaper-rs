use crate::callers::{Caller, caller};
use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, denied, failed, invalid};
use crate::inputs::{Control, Grants, Hub, View, declined_path, grants_path, hub, load, load_bound};
use crate::items::reply;
use crate::streams::OutputInfo;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Weak};
use wprs_core::model::{Input, InputsReply, MODEL};
use zbus::message::Header;

pub struct Inputs(pub Weak<Daemon>);

pub(crate) fn scene_id(scene: &str) -> Result<&str> {
    let valid = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c);
    let fine = (1..=255).contains(&scene.len()) && scene.chars().all(valid);
    fine.then_some(scene).ok_or_else(|| invalid(format!("{scene} is not a scene package id")))
}

pub(crate) fn start(daemon: &Arc<Daemon>, connection: zbus::Connection) {
    let path = grants_path(daemon);
    crate::inputs::start(daemon, connection, (load_bound(&path), load(&declined_path(&path))));
}

impl Daemon {
    pub(crate) fn views(&self) -> BTreeMap<String, View> {
        let st = self.state();
        let view = |name: &String, info: &OutputInfo| View {
            active: st.active(info) && !st.locked,
            covered: info.covered(),
            rect: info.screen.as_ref().map(|s| [s.x.into(), s.y.into(), s.width.into(), s.height.into()]),
            clicks: st.config.clicks(name),
        };
        st.outputs.iter().map(|(name, info)| (name.clone(), view(name, info))).collect()
    }
}

impl Inputs {
    fn parts(&self) -> Result<(Arc<Daemon>, &'static Arc<Hub>)> {
        let stopping = || failed("the daemon is stopping");
        Ok((self.0.upgrade().ok_or_else(stopping)?, hub().ok_or_else(stopping)?))
    }

    async fn give(
        &self,
        call: (&zbus::Connection, &Header<'_>),
        scene: &str,
        key: Option<&str>,
        input: &str,
    ) -> Result<()> {
        let (d, hub) = self.parts()?;
        let (input, scene) = (parse(input)?, scene_id(scene)?.to_owned());
        let (key, title) = lock(&hub.control).package(&scene, key)?;
        if !matches!(caller(call.0, call.1).await, Caller::Ctl | Caller::Ours) {
            if lock(&hub.control).has(&scene, &key, input) {
                return Ok(());
            }
            return crate::confirm::grant(call.0, hub.clone(), grants_path(&d), (scene, key, title), input)
                .await;
        }
        d.run(move |d| hub.grant(&grants_path(d), &scene, &key, input)).await
    }
}

pub(crate) fn reported(control: &Control) -> Grants {
    let reported = |(id, keys): (&String, &Grants)| {
        let shown: Vec<&str> = control
            .shown
            .values()
            .filter(|s| s.scene == *id && !s.key.is_empty())
            .map(|s| s.key.as_str())
            .collect();
        let keys = keys.iter().filter(|(key, _)| shown.is_empty() || shown.contains(&key.as_str()));
        let inputs: BTreeSet<Input> = keys.flat_map(|(_, inputs)| inputs).copied().collect();
        (!inputs.is_empty()).then(|| (id.clone(), inputs))
    };
    control.grants.iter().filter_map(reported).collect()
}

fn parse(input: &str) -> Result<Input> {
    Input::parse(input).ok_or_else(|| invalid(format!("{input} is not audio, pointer or clicks")))
}

#[zbus::interface(name = "io.github.wallpaper_rs.Inputs1")]
impl Inputs {
    async fn grant(
        &self,
        scene: String,
        input: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] bus: &zbus::Connection,
    ) -> Result<()> {
        self.give((bus, &header), &scene, None, &input).await
    }

    async fn grant_package(
        &self,
        scene: String,
        key: String,
        input: String,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] bus: &zbus::Connection,
    ) -> Result<()> {
        self.give((bus, &header), &scene, Some(&key), &input).await
    }

    async fn revoke(&self, scene: String, input: String) -> Result<()> {
        let (d, hub) = self.parts()?;
        let (input, scene) = (parse(&input)?, scene_id(&scene)?.to_owned());
        d.run(move |d| hub.revoke(&grants_path(d), &scene, input)).await
    }

    #[zbus(out_args("json"))]
    async fn get_inputs(&self) -> Result<String> {
        let (d, hub) = self.parts()?;
        let (serial, tracking) = (d.changes.serial(), *hub.tracking.borrow());
        let control = lock(&hub.control);
        let grants = reported(&control);
        let mut shown = control.shown.clone();
        shown.values_mut().for_each(|s| s.granted = control.granted(&s.scene, &s.key));
        let (parallax, declined, capturing) =
            (control.parallax.clone(), control.declined.clone(), control.capturing);
        let clicks = control.views.iter().filter(|(_, v)| v.clicks).map(|(name, _)| name.clone()).collect();
        let clicking = control.wants(Input::Clicks);
        drop(control);
        let (asking, quiet) = crate::confirm::questions();
        reply(InputsReply {
            model: MODEL,
            serial,
            capturing,
            tracking,
            grants,
            shown,
            parallax,
            declined,
            asking,
            quiet,
            clicks,
            clicking,
        })
    }

    async fn set_clicks(
        &self,
        output: String,
        on: bool,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] bus: &zbus::Connection,
    ) -> Result<()> {
        let (d, _) = self.parts()?;
        if on && !matches!(caller(bus, &header).await, Caller::Ctl | Caller::Ours) {
            return Err(denied("only the app, the tray and wprs-ctl let clicks reach a wallpaper"));
        }
        let set = move |c: &mut wprs_core::Config| {
            c.set_clicks(output.trim(), on);
            Ok(())
        };
        d.run(move |d| d.update_config(set)).await
    }

    async fn touch(
        &self,
        output: String,
        x: f64,
        y: f64,
        buttons: u32,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] bus: &zbus::Connection,
    ) -> Result<()> {
        if caller(bus, &header).await != Caller::Ours || crate::flatpak::app_id().is_some() {
            return Err(denied("only the Plasma wallpaper of wprsd reports clicks"));
        }
        if !lock(&self.parts()?.1.control).wants(Input::Clicks).contains(&output) {
            return Err(denied(format!("{output} does not take clicks now")));
        }
        let inside = [x, y].iter().all(|v| (0.0..=1.0).contains(v));
        let pos = inside.then_some([x as f32, y as f32]);
        crate::inputs::surface_pointer(&output, pos, buttons.min(255) as u8);
        Ok(())
    }

    async fn set_pointer(
        &self,
        x: i32,
        y: i32,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] bus: &zbus::Connection,
    ) -> Result<()> {
        if !crate::callers::reports_pointer(caller(bus, &header).await) {
            return Err(denied(match crate::flatpak::app_id() {
                Some(_) => {
                    "only the KWin script of wprsd reports the pointer: inside the Flatpak, wprs-ctl cannot be told apart from other programs"
                }
                None => "only the KWin script of wprsd and wprs-ctl report the pointer",
            }));
        }
        self.parts()?.1.set_pointer(x.into(), y.into());
        Ok(())
    }
}
