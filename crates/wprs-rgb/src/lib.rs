mod client;
mod paint;
mod read;

pub use client::{Client, Error};
pub use paint::{Area, Flow, Strip, colors, pattern, shrink};
pub use read::{Device, Mode, Zone, controller, keyed};

pub const PORT: u16 = 6742;
pub const VERSION: u32 = 4;
pub const MAX_DEVICES: u32 = 64;
pub const MAX_LEDS: usize = 4096;
pub const MAX_PACKET: u32 = 1 << 20;
pub const STRIP: (u32, u32) = (64, 16);

pub(crate) const MAGIC: &[u8; 4] = b"ORGB";
pub(crate) const COUNT: u32 = 0;
pub(crate) const DATA: u32 = 1;
pub(crate) const PROTOCOL: u32 = 40;
pub(crate) const NAME: u32 = 50;
pub(crate) const UPDATED: u32 = 100;
pub(crate) const LEDS: u32 = 1050;
pub(crate) const CUSTOM: u32 = 1100;
pub(crate) const MODE: u32 = 1101;

pub fn kind(code: i32) -> &'static str {
    const KINDS: [&str; 19] = [
        "motherboard",
        "memory",
        "graphics card",
        "cooler",
        "light strip",
        "keyboard",
        "mouse",
        "mouse pad",
        "headset",
        "headset stand",
        "gamepad",
        "light",
        "speaker",
        "virtual",
        "storage",
        "case",
        "microphone",
        "accessory",
        "keypad",
    ];
    usize::try_from(code).ok().and_then(|i| KINDS.get(i)).copied().unwrap_or("other")
}

pub fn header(device: u32, id: u32, size: u32) -> [u8; 16] {
    let mut out = [0; 16];
    out[..4].copy_from_slice(MAGIC);
    out[4..8].copy_from_slice(&device.to_le_bytes());
    out[8..12].copy_from_slice(&id.to_le_bytes());
    out[12..].copy_from_slice(&size.to_le_bytes());
    out
}

pub fn leds(colors: &[[u8; 3]]) -> Vec<u8> {
    let n = colors.len().min(usize::from(u16::MAX));
    let size = (4 + 2 + 4 * n) as u32;
    let mut out = Vec::with_capacity(size as usize);
    out.extend(size.to_le_bytes());
    out.extend((n as u16).to_le_bytes());
    colors[..n].iter().for_each(|[r, g, b]| out.extend([*r, *g, *b, 0]));
    out
}
