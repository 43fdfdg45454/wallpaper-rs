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

pub fn volume() -> String {
    tr("Volume")
}

pub fn video() -> [String; 8] {
    let (speed, start, stop, volume) = (tr("Speed"), tr("Start"), tr("Stop at"), tr("Volume"));
    let (zoom, across, down, end) =
        (tr("Zoom"), tr("Horizontal position"), tr("Vertical position"), tr("0 plays to the end"));
    [speed, start, stop, volume, zoom, across, down, end]
}
