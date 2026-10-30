use crate::choice::choose;
use crate::daemon::Daemon;
use crate::error::Result;
use crate::playlists1::library;
use std::collections::HashMap;
use std::sync::{Arc, Weak};

pub struct Profiles(pub Weak<Daemon>);

#[zbus::interface(name = "io.github.wallpaper_rs.Profiles1")]
impl Profiles {
    #[zbus(out_args("json"))]
    async fn list(&self) -> Result<String> {
        library(&self.0)?.run(|d| d.list_profiles()).await
    }

    #[zbus(out_args("json"))]
    async fn get(&self, profile: String) -> Result<String> {
        library(&self.0)?.run(move |d| d.get_profile(&profile)).await
    }

    #[zbus(out_args("uid"))]
    async fn save(&self, name: String, with_options: bool) -> Result<String> {
        library(&self.0)?.run(move |d| d.save_profile(&name, with_options)).await
    }

    async fn activate(&self, profile: String) -> Result<()> {
        let chosen =
            move |d: &Arc<Daemon>| choose(|| Ok(Some(profile)), |p| d.activate(p).map(|()| d.hold(true)));
        library(&self.0)?.render(chosen).await
    }

    async fn rename(&self, profile: String, name: String) -> Result<()> {
        library(&self.0)?
            .run(move |d| {
                d.edit_profile(&profile, |p| {
                    p.name = name;
                    Ok(())
                })
            })
            .await
    }

    async fn delete(&self, profile: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.delete_profile(&profile)).await
    }

    async fn set_auto(&self, profile: String, key: String, value: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.set_auto(&profile, &key, &value)).await
    }

    #[zbus(out_args("json"))]
    async fn list_presets(&self, item: String) -> Result<String> {
        library(&self.0)?.run(move |d| d.list_presets(&item)).await
    }

    #[zbus(out_args("uid"))]
    async fn save_preset(
        &self,
        item: String,
        name: String,
        values: HashMap<String, String>,
    ) -> Result<String> {
        library(&self.0)?.run(move |d| d.save_preset(&item, &name, &values)).await
    }

    async fn delete_preset(&self, preset: String) -> Result<()> {
        library(&self.0)?.run(move |d| d.delete_preset(&preset)).await
    }
}
