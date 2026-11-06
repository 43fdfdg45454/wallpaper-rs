use std::path::Path;
use wgpu::DeviceType;

const AMD: u32 = 0x1002;
const NVIDIA: u32 = 0x10de;
const INTEL: u32 = 0x8086;
const CARVED: u64 = 2 << 30;

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|text| text.trim().to_owned())
}

fn bus(device: &Path) -> Option<u8> {
    let real = std::fs::canonicalize(device).ok()?;
    let name = real.file_name()?.to_str()?;
    let mut parts = name.split(':');
    let (_, bus) = (parts.next()?, parts.next()?);
    u8::from_str_radix(bus, 16).ok()
}

fn hex(path: &Path) -> Option<u32> {
    u32::from_str_radix(read(path)?.trim_start_matches("0x"), 16).ok()
}

fn dedicated_intel(id: u32) -> bool {
    matches!(id >> 8, 0x49 | 0x56 | 0xe2)
}

fn card(device: &Path) -> Option<Card> {
    let vendor = hex(&device.join("vendor"))?;
    let memory = read(&device.join("mem_info_vram_total")).and_then(|v| v.parse::<u64>().ok());
    let id = hex(&device.join("device"));
    let kind = match vendor {
        INTEL if bus(device) == Some(0) => DeviceType::IntegratedGpu,
        INTEL if bus(device).is_some() => DeviceType::DiscreteGpu,
        INTEL if id.is_some_and(dedicated_intel) => DeviceType::DiscreteGpu,
        INTEL if id.is_some() => DeviceType::IntegratedGpu,
        NVIDIA => DeviceType::DiscreteGpu,
        AMD if memory.is_some_and(|m| m <= CARVED) => DeviceType::IntegratedGpu,
        AMD if memory.is_some() => DeviceType::DiscreteGpu,
        _ => DeviceType::Other,
    };
    Some(Card { vendor, kind, boot: read(&device.join("boot_vga")).as_deref() == Some("1") })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Card {
    pub(crate) vendor: u32,
    pub(crate) kind: DeviceType,
    pub(crate) boot: bool,
}

pub(crate) fn cards(root: &Path) -> Vec<Card> {
    let entries = std::fs::read_dir(root.join("class/drm")).into_iter().flatten().flatten();
    let named = |name: &str| {
        name.strip_prefix("card").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    let mut names: Vec<String> =
        entries.map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| named(n)).collect();
    names.sort();
    names.iter().filter_map(|n| card(&root.join("class/drm").join(n).join("device"))).collect()
}

pub(crate) fn guess(vendor: u32, cards: &[Card], prime: bool) -> DeviceType {
    let same: Vec<Card> = match cards {
        [only] => vec![*only],
        _ => cards.iter().filter(|c| c.vendor == vendor).copied().collect(),
    };
    let one = |kinds: &[DeviceType]| match kinds.first() {
        Some(kind) if kinds.iter().all(|k| k == kind) => Some(*kind),
        _ => None,
    };
    let kinds = |boot: bool| same.iter().filter(|c| c.boot == boot).map(|c| c.kind).collect::<Vec<_>>();
    let all: Vec<DeviceType> = same.iter().map(|c| c.kind).collect();
    let used = if same.iter().any(|c| c.boot) { one(&kinds(!prime)) } else { None };
    one(&all).or(used).unwrap_or(DeviceType::Other)
}

fn prime() -> bool {
    std::env::var("DRI_PRIME").is_ok_and(|v| !v.is_empty() && v != "0")
}

pub(crate) fn kind_of(info: &wgpu::AdapterInfo) -> DeviceType {
    match (info.backend, info.device_type) {
        (wgpu::Backend::Gl, DeviceType::Other) => guess(info.vendor, &cards(Path::new("/sys")), prime()),
        (_, kind) => kind,
    }
}

#[cfg(test)]
mod tests;
