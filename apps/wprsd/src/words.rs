use std::collections::HashMap;
use std::sync::LazyLock;
use wprs_core::locale::{catalog, languages};

const CATALOGS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));
static CATALOG: LazyLock<HashMap<String, String>> = LazyLock::new(|| catalog(CATALOGS, &languages()));

fn tr(text: &str) -> String {
    CATALOG.get(text).cloned().unwrap_or_else(|| text.to_owned())
}

pub fn conflict(name: &str) -> String {
    tr("%1 (conflict)").replace("%1", name)
}

pub fn too_old() -> String {
    tr(
        "This account is synced by a newer version of wallpaper-rs: update it on this computer to keep syncing.",
    )
}

pub fn same_passphrase() -> String {
    tr(
        "This is the recovery passphrase from before a device was revoked, and that device may know it: choose a new, different passphrase.",
    )
}

pub fn flashing_moves() -> String {
    tr("A package that flashes cannot keep moving when the desktop asks for reduced motion.")
}

pub fn volume() -> String {
    tr("Volume")
}

pub fn video() -> [String; 8] {
    let (speed, start, stop, volume) = (tr("Speed"), tr("Start"), tr("Stop at"), tr("Volume"));
    let (zoom, across, down, end) =
        (tr("Zoom"), tr("Horizontal position"), tr("Vertical position"), tr("0 plays to the end"));
    [speed, start, stop, volume, zoom, across, down, end]
}

pub fn scripts_off() -> String {
    tr("The scripts of this scene failed three times in a row and are off; the scene goes on without them.")
}

pub fn script_stopped(why: &str) -> String {
    tr("A script of this scene stopped and the scene goes on without it: %1").replace("%1", why)
}

pub fn answers() -> (String, String) {
    (tr("Allow"), tr("Not now"))
}

pub fn request(title: &str, audio: bool) -> (String, String) {
    let safe = |c| match c {
        '<' => '‹',
        '>' => '›',
        '&' => '＆',
        c => c,
    };
    let title: String = title.chars().map(safe).collect();
    match audio {
        true => (
            tr("Let a scene follow the sound?"),
            tr("Let “%1” follow the sound that plays? The sound is turned into levels on this computer; nothing is recorded, kept or sent.").replace("%1", &title),
        ),
        false => (
            tr("Let a scene follow the pointer?"),
            tr("Let “%1” follow the pointer? Only its latest position over the screen reaches the scene, and it is never kept.").replace("%1", &title),
        ),
    }
}

pub fn web_missing() -> String {
    tr("Web wallpapers cannot be shown here: the web helper wprs-web is not installed.")
}

pub fn web_engine() -> String {
    tr("Web wallpapers cannot be shown here: they need WPE WebKit 2.54 or newer, which is not installed.")
}

pub fn web_old(why: &str) -> String {
    tr("Web wallpapers cannot be shown here: they need WPE WebKit 2.54 or newer (%1).").replace("%1", why)
}

pub fn web_broken(why: &str) -> String {
    tr("Web wallpapers cannot be shown here: the web helper does not start (%1).").replace("%1", why)
}

pub fn web_failed() -> String {
    tr("The web page stopped three times in a row and is off; the background color shows instead.")
}

pub fn web_stalled() -> String {
    tr("The web page drew nothing for 10 seconds and is off; the background color shows instead.")
}

pub fn web_full() -> String {
    tr(
        "The web page keeps much more data than it declared, so it runs without keeping anything until its data is cleared.",
    )
}

pub fn web_crashed() -> String {
    tr(
        "The web page crashed or stopped answering four times in a row and is off; the background color shows instead.",
    )
}

pub fn gpu_missing(name: &str) -> String {
    tr("No graphics card matches “%1”; the automatic choice is used.").replace("%1", name)
}

pub fn gpu_unusable(name: &str) -> String {
    tr("The graphics card chosen by “%1” cannot be opened; the automatic choice is used.").replace("%1", name)
}
