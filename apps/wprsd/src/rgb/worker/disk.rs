use std::path::Path;
use wprs_rgb::{Device, MAX_DEVICES, MAX_LEDS, Mode};

const MAX_MODE: usize = 64 << 10;

#[derive(serde::Serialize, serde::Deserialize)]
struct Kept {
    index: u32,
    key: String,
    name: String,
    leds: u32,
    mode: Option<(i32, String, Vec<u8>, bool)>,
    colors: Vec<[u8; 3]>,
}

fn fits(k: &Kept) -> bool {
    let mode = k
        .mode
        .as_ref()
        .is_none_or(|(i, name, data, _)| *i >= 0 && name.len() <= 512 && data.len() <= MAX_MODE);
    let named = |s: &str| (1..=256).contains(&s.chars().count()) && !s.chars().any(char::is_control);
    mode && named(&k.key) && named(&k.name) && k.colors.len() <= MAX_LEDS && k.leds as usize <= MAX_LEDS
}

pub(super) fn load(path: &Path) -> Vec<Device> {
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    let kept: Vec<Kept> = serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        tracing::warn!("ignoring unreadable RGB modes: {e}");
        Vec::new()
    });
    let device = |k: Kept| Device {
        index: k.index,
        key: k.key,
        name: k.name,
        leds: k.leds,
        mode: k.mode.map(|(index, name, data, per_led)| Mode { index, name, data, per_led }),
        colors: k.colors,
        ..Device::default()
    };
    kept.into_iter()
        .filter(|k| k.index < MAX_DEVICES && fits(k))
        .take(MAX_DEVICES as usize)
        .map(device)
        .collect()
}

pub(super) fn save(path: &Path, saved: &[Device]) {
    let kept: Vec<Kept> = saved
        .iter()
        .map(|d| Kept {
            index: d.index,
            key: d.key.clone(),
            name: d.name.clone(),
            leds: d.leds,
            mode: d.mode.as_ref().map(|m| (m.index, m.name.clone(), m.data.clone(), m.per_led)),
            colors: d.colors.clone(),
        })
        .collect();
    let done = match kept.is_empty() {
        true => std::fs::remove_file(path).or_else(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(e),
        }),
        false => crate::inputs::store::save(path, &kept),
    };
    if let Err(e) = done {
        tracing::warn!("cannot keep the own modes of the RGB devices: {e}");
    }
}
