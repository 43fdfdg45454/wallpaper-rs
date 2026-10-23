use crate::daemon::Daemon;
use crate::error::{Result, invalid};
use crate::sceneprops::{Props, Stored, stored};
use serde_json::Value as Json;
use std::path::Path;
use wprs_scene::{Control, Values};

impl Daemon {
    pub fn set_scene_property(&self, output: &str, id: &str, value: &str) -> Result<()> {
        let (output, props) = self.props_on(output)?;
        let prop =
            props.properties.get(id).ok_or_else(|| invalid(format!("{} has no setting {id:?}", props.id)))?;
        if !prop.holds_value() || matches!(prop.control, Control::Text { secret: true, .. }) {
            return Err(invalid(format!("{id} is a {} and holds no stored value", prop.kind())));
        }
        let given: Json =
            serde_json::from_str(value).map_err(|e| invalid(format!("the value is not JSON: {e}")))?;
        let checked = prop.value(&given).map_err(|e| invalid(format!("{id} {e}")))?;
        if checked != given {
            tracing::warn!(property = id, "a property value was adjusted to its range");
        }
        if let Some(path) = checked.as_str().filter(|_| prop.personal()) {
            self.check_choice(prop, Path::new(path))
                .map_err(|e| invalid(format!("{id}: cannot use {path}: {e:#}")))?;
        }
        self.store_values(&output, &props, |values| match stored(&checked) {
            Some(value) => drop(values.insert(id.to_owned(), value)),
            None => drop(values.remove(id)),
        })
    }

    fn check_choice(&self, prop: &wprs_scene::Property, path: &Path) -> anyhow::Result<()> {
        if let Control::Folder { .. } = prop.control {
            anyhow::ensure!(path.is_dir(), "it is not a folder");
            return Ok(());
        }
        let video = wprs_video::is_video(path);
        anyhow::ensure!(
            prop.accepts(if video { "video" } else { "image" }),
            "this property takes no such file"
        );
        match video {
            true => wprs_video::probe(path).map(drop).map_err(Into::into),
            false => self.scene_file(&path.to_string_lossy()).map(drop),
        }
    }

    pub fn reset_scene_properties(&self, output: &str) -> Result<()> {
        let (output, props) = self.props_on(output)?;
        self.store_values(&output, &props, Stored::clear)
    }

    pub fn share_scene_values(&self, output: &str, shared: bool) -> Result<()> {
        let (output, props) = self.props_on(output)?;
        if output.is_empty() {
            return Err(invalid("name a screen: the default section is what every screen shares"));
        }
        self.update_config(|c| {
            let own = c.outputs.get_mut(&output).and_then(|o| o.scenes.remove(&props.id));
            match (shared, own) {
                (true, Some(own)) => c.default.scenes.entry(props.id.clone()).or_default().extend(own),
                (false, own) => {
                    let copied = own.or_else(|| c.default.scenes.get(&props.id).cloned()).unwrap_or_default();
                    c.scene_section(&output).insert(props.id.clone(), copied);
                }
                (true, None) => {}
            }
            Ok(())
        })?;
        self.refresh_package(&props.path);
        Ok(())
    }

    pub fn store_values(&self, output: &str, props: &Props, change: impl FnOnce(&mut Stored)) -> Result<()> {
        self.update_config(|c| {
            let own = c.outputs.get(output).is_some_and(|o| o.scenes.contains_key(&props.id));
            let section = c.scene_section(if own { output } else { "" });
            let values = section.entry(props.id.clone()).or_default();
            change(values);
            if values.is_empty() && !own {
                section.remove(&props.id);
            }
            Ok(())
        })?;
        self.refresh_package(&props.path);
        Ok(())
    }

    pub fn refresh_package(&self, path: &Path) {
        let shows = |info: &crate::streams::OutputInfo| {
            let scene = info.scene.as_ref().is_some_and(|s| s.path == path);
            scene || info.video.as_ref().is_some_and(|v| v.path == path)
        };
        let affected: Vec<String> =
            self.state().outputs.iter().filter(|(_, i)| shows(i)).map(|(n, _)| n.clone()).collect();
        affected.iter().for_each(|o| drop(self.configure_output(o)));
    }

    pub fn keep_values(&self, output: &str, props: &Props, values: &Values) -> Result<()> {
        let fit = |(k, v): (&String, &Json)| {
            let prop = props.properties.get(k).filter(|p| p.holds_value() && !p.personal())?;
            Some((k.clone(), stored(&prop.value(v).ok()?)?))
        };
        let chosen: Vec<_> = values.iter().filter_map(fit).collect();
        self.store_values(output, props, |values| values.extend(chosen))
    }

    pub fn press_scene_button(&self, output: &str, id: &str) -> Result<()> {
        let (output, props) = self.props_on(output)?;
        let button = props.properties.get(id).filter(|p| matches!(p.control, Control::Button {}));
        let button = button.ok_or_else(|| invalid(format!("{} has no button {id:?}", props.id)))?;
        let st = self.state();
        let values = props.values(&st.config.scene_values(&output, &props.id));
        if !button.met(&props.properties, &values, |p| &p.enabled_if) {
            return Err(invalid(format!("{id} cannot be pressed while its condition does not hold")));
        }
        let shows = |(name, info): &(&String, &crate::streams::OutputInfo)| {
            (output.is_empty() || **name == output)
                && info.scene.as_ref().is_some_and(|s| s.path == props.path)
        };
        let shown: Vec<&String> = st.outputs.iter().filter(shows).map(|(n, _)| n).collect();
        shown.iter().for_each(|name| self.compositor.press(name, id));
        Ok(())
    }
}
