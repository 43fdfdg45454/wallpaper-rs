use std::collections::HashMap;
use zbus::{Result, proxy};

#[proxy(
    interface = "io.github.wallpaper_rs.Daemon1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Daemon1 {
    fn request_stream(&self, output: &str, width: u32, height: u32) -> Result<(u32, u64)>;
    fn hold_stream(&self, output: &str, width: u32, height: u32) -> Result<(u32, u64)>;
    fn release_stream(&self, output: &str) -> Result<()>;
    fn set_wallpaper(&self, path: &str, output: &str, fit: &str) -> Result<()>;
    fn set_background(&self, color: &str, output: &str) -> Result<()>;
    fn set_option(&self, key: &str, value: &str) -> Result<()>;
    fn set_paused(&self, paused: bool) -> Result<()>;
    fn set_volume(&self, volume: f64) -> Result<()>;
    fn set_muted(&self, muted: bool) -> Result<()>;
    fn set_output_covered(&self, output: &str, covered: bool) -> Result<()>;
    #[allow(clippy::too_many_arguments)]
    fn set_output_info(
        &self,
        output: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        scale: f64,
        px_per_mm: f64,
        identity: &str,
    ) -> Result<()>;
    fn preview_edit(&self, path: &str, edit_json: &str) -> Result<()>;
    fn get_scene_properties(&self, output: &str) -> Result<String>;
    fn set_scene_property(&self, output: &str, id: &str, value_json: &str) -> Result<()>;
    fn apply_scene_preset(&self, output: &str, preset: &str) -> Result<()>;
    fn reset_scene_properties(&self, output: &str) -> Result<()>;
    fn reload(&self) -> Result<()>;
    fn get_state(&self) -> Result<String>;
    fn get_schema(&self) -> Result<String>;
    fn wait_changed(&self, serial: u64, timeout_ms: u32) -> Result<(u64, Vec<String>)>;
    fn list_jobs(&self) -> Result<String>;
    fn get_job(&self, job: u64) -> Result<String>;
    fn cancel_job(&self, job: u64) -> Result<()>;
    #[zbus(signal)]
    fn changed(&self, topic: &str, ids: Vec<String>, serial: u64) -> Result<()>;
    #[zbus(signal)]
    fn job_progress(&self, job: u64, done: u64, total: u64) -> Result<()>;
    #[zbus(signal)]
    fn job_finished(&self, job: u64, state: &str, error_name: &str, message: &str) -> Result<()>;
    #[zbus(property(emits_changed_signal = "const"))]
    fn version(&self) -> Result<String>;
    #[zbus(property(emits_changed_signal = "const"))]
    fn adapter(&self) -> Result<String>;
    #[zbus(property)]
    fn paused(&self) -> Result<bool>;
    #[zbus(property)]
    fn volume(&self) -> Result<f64>;
    #[zbus(property)]
    fn muted(&self) -> Result<bool>;
    #[zbus(property(emits_changed_signal = "false"))]
    fn serial(&self) -> Result<u64>;
    #[zbus(property(emits_changed_signal = "const"))]
    fn model(&self) -> Result<u32>;
}

#[proxy(
    interface = "io.github.wallpaper_rs.Library1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Library1 {
    fn add_source(&self, path: &str, recursive: bool) -> Result<(String, u64)>;
    fn remove_source(&self, source: &str) -> Result<()>;
    fn list_sources(&self) -> Result<String>;
    fn rescan(&self, source: &str) -> Result<u64>;
    fn import(&self, paths: &[&str], copy: bool) -> Result<u64>;
    fn resolve(&self, refs: &[&str]) -> Result<Vec<String>>;
    fn remove(&self, uids: &[&str], delete_files: bool) -> Result<()>;
    fn search(&self, query: &str, offset: u32, limit: u32) -> Result<String>;
    fn get_item(&self, uid: &str) -> Result<String>;
    fn set_title(&self, uid: &str, title: &str) -> Result<()>;
    fn set_rating(&self, uids: &[&str], rating: u32) -> Result<()>;
    fn set_favorite(&self, uids: &[&str], favorite: bool) -> Result<()>;
    fn set_tags(&self, uids: &[&str], add: &[&str], remove: &[&str]) -> Result<()>;
    fn list_tags(&self) -> Result<String>;
    fn rename_tag(&self, from: &str, to: &str) -> Result<()>;
    fn thumbnail(&self, uid: &str, size: u32) -> Result<String>;
    fn request_thumbnails(&self, uids: &[&str], size: u32) -> Result<HashMap<String, String>>;
    fn apply(&self, uid: &str, output: &str, fit: &str, preset: &str) -> Result<()>;
    #[zbus(signal)]
    fn thumbnail_ready(&self, uid: &str, size: u32, path: &str) -> Result<()>;
}

#[proxy(
    interface = "io.github.wallpaper_rs.Playlists1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Playlists1 {
    fn list(&self) -> Result<String>;
    fn get(&self, playlist: &str, offset: u32, limit: u32) -> Result<String>;
    fn create(&self, name: &str, query: &str) -> Result<String>;
    fn rename(&self, playlist: &str, name: &str) -> Result<()>;
    fn set_query(&self, playlist: &str, query: &str) -> Result<()>;
    fn delete(&self, playlist: &str) -> Result<()>;
    fn add_items(&self, playlist: &str, uids: &[&str]) -> Result<()>;
    fn remove_items(&self, playlist: &str, uids: &[&str]) -> Result<()>;
    fn remove_entries(&self, playlist: &str, indexes: &[u32], expected: &[&str]) -> Result<()>;
    fn move_item(&self, playlist: &str, from: u32, to: u32, expected: &str) -> Result<()>;
    fn set_rule(&self, playlist: &str, key: &str, value: &str) -> Result<()>;
    fn start(&self, playlist: &str, output: &str) -> Result<()>;
    fn stop(&self, output: &str) -> Result<()>;
    fn next(&self, output: &str) -> Result<()>;
    fn previous(&self, output: &str) -> Result<()>;
    fn history(&self, output: &str, limit: u32) -> Result<String>;
}

#[proxy(
    interface = "io.github.wallpaper_rs.Profiles1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Profiles1 {
    fn list(&self) -> Result<String>;
    fn get(&self, profile: &str) -> Result<String>;
    fn save(&self, name: &str, with_options: bool) -> Result<String>;
    fn activate(&self, profile: &str) -> Result<()>;
    fn rename(&self, profile: &str, name: &str) -> Result<()>;
    fn delete(&self, profile: &str) -> Result<()>;
    fn set_auto(&self, profile: &str, key: &str, value: &str) -> Result<()>;
    fn list_presets(&self, item: &str) -> Result<String>;
    fn save_preset(&self, item: &str, name: &str, values: HashMap<&str, &str>) -> Result<String>;
    fn delete_preset(&self, preset: &str) -> Result<()>;
}

#[proxy(
    interface = "io.github.wallpaper_rs.Providers1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Providers1 {
    fn list(&self) -> Result<String>;
    fn add(&self, id: &str, kind: &str, options: HashMap<&str, &str>, secret: &str) -> Result<String>;
    fn set(&self, id: &str, key: &str, value: &str) -> Result<()>;
    fn remove(&self, id: &str, purge: bool) -> Result<()>;
    fn sync(&self, id: &str) -> Result<u64>;
    fn set_secret(&self, id: &str, secret: &str) -> Result<()>;
    fn forget_secret(&self, id: &str) -> Result<()>;
}

#[proxy(
    interface = "io.github.wallpaper_rs.Repos1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Repos1 {
    fn list(&self) -> Result<String>;
    fn add(&self, url: &str) -> Result<u64>;
    fn refresh(&self, repo: &str) -> Result<u64>;
    fn remove(&self, repo: &str, uninstall: bool) -> Result<()>;
    fn browse(&self, query: &str, offset: u32, limit: u32) -> Result<String>;
    fn request_previews(&self, specs: &[&str]) -> Result<HashMap<String, String>>;
    fn install(&self, spec: &str, allow_stale: bool, allow_downgrade: bool) -> Result<u64>;
    fn uninstall(&self, id: &str, version: &str) -> Result<()>;
    fn list_installed(&self) -> Result<String>;
    #[zbus(signal)]
    fn preview_ready(&self, spec: &str, path: &str) -> Result<()>;
}

#[proxy(
    interface = "io.github.wallpaper_rs.Inputs1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Inputs1 {
    fn grant(&self, scene: &str, input: &str) -> Result<()>;
    fn revoke(&self, scene: &str, input: &str) -> Result<()>;
    fn get_inputs(&self) -> Result<String>;
    fn set_pointer(&self, x: i32, y: i32) -> Result<()>;
}

#[proxy(
    interface = "io.github.wallpaper_rs.Sync1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Sync1 {
    fn add_server(&self, url: &str) -> Result<u64>;
    fn link(&self, name: &str, plaintext: bool) -> Result<(String, String, u64)>;
    fn preview_code(&self, user_code: &str) -> Result<u64>;
    fn approve(&self, user_code: &str) -> Result<u64>;
    fn get_status(&self) -> Result<String>;
    fn sync_now(&self) -> Result<u64>;
    fn set_paused(&self, paused: bool) -> Result<()>;
    fn join(&self, device: &str) -> Result<u64>;
    fn share_key(&self, device: &str) -> Result<u64>;
    fn confirm_share(&self, device: &str, code: &str) -> Result<u64>;
    fn rotate(&self) -> Result<u64>;
    fn reset(&self) -> Result<u64>;
    fn confirm_repo(&self, repo: &str, add: bool) -> Result<u64>;
    fn share_secret(&self, provider: &str, share: bool) -> Result<()>;
    fn set_escrow(&self, passphrase: &str) -> Result<u64>;
    fn restore(&self, passphrase: &str) -> Result<u64>;
    fn share_files(&self, targets: &[&str], share: bool) -> Result<u64>;
    fn list_files(&self) -> Result<String>;
    fn fetch_file(&self, file: &str) -> Result<u64>;
    fn upload(&self, path: &str) -> Result<u64>;
    fn remove_server(&self, wipe: bool) -> Result<()>;
}
