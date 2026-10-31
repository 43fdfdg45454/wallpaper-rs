use zbus::{Result, proxy};

#[proxy(
    interface = "io.github.wallpaper_rs.Inputs1",
    default_service = "io.github.wallpaper_rs.Daemon",
    default_path = "/io/github/wallpaper_rs/Daemon"
)]
pub trait Inputs1 {
    fn grant(&self, scene: &str, input: &str) -> Result<()>;
    fn grant_package(&self, scene: &str, key: &str, input: &str) -> Result<()>;
    fn revoke(&self, scene: &str, input: &str) -> Result<()>;
    fn get_inputs(&self) -> Result<String>;
    fn set_pointer(&self, x: i32, y: i32) -> Result<()>;
    fn set_clicks(&self, output: &str, on: bool) -> Result<()>;
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
    fn approve_device(&self, user_code: &str, password: &str) -> Result<u64>;
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
