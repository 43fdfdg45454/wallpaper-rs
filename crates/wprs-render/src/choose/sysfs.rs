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

fn card(device: &Path) -> Option<(u32, DeviceType)> {
    let vendor = u32::from_str_radix(read(&device.join("vendor"))?.trim_start_matches("0x"), 16).ok()?;
    let memory = read(&device.join("mem_info_vram_total")).and_then(|v| v.parse::<u64>().ok());
    let kind = match vendor {
        INTEL if bus(device) == Some(0) => DeviceType::IntegratedGpu,
        INTEL if bus(device).is_some() => DeviceType::DiscreteGpu,
        NVIDIA => DeviceType::DiscreteGpu,
        AMD if memory.is_some_and(|m| m <= CARVED) => DeviceType::IntegratedGpu,
        AMD if memory.is_some() => DeviceType::DiscreteGpu,
        _ => DeviceType::Other,
    };
    Some((vendor, kind))
}

pub(crate) fn cards(root: &Path) -> Vec<(u32, DeviceType)> {
    let entries = std::fs::read_dir(root.join("class/drm")).into_iter().flatten().flatten();
    let named = |name: &str| {
        name.strip_prefix("card").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    let mut names: Vec<String> =
        entries.map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| named(n)).collect();
    names.sort();
    names.iter().filter_map(|n| card(&root.join("class/drm").join(n).join("device"))).collect()
}

pub(crate) fn guess(vendor: u32, cards: &[(u32, DeviceType)]) -> DeviceType {
    let same: Vec<DeviceType> = match cards {
        [(_, only)] => vec![*only],
        _ => cards.iter().filter(|(v, _)| *v == vendor).map(|(_, k)| *k).collect(),
    };
    match same.first() {
        Some(kind) if same.iter().all(|k| k == kind) => *kind,
        _ => DeviceType::Other,
    }
}

pub(crate) fn kind_of(info: &wgpu::AdapterInfo) -> DeviceType {
    match (info.backend, info.device_type) {
        (wgpu::Backend::Gl, DeviceType::Other) => guess(info.vendor, &cards(Path::new("/sys"))),
        (_, kind) => kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, card: &str, vendor: &str, memory: Option<u64>) {
        let device = root.join("class/drm").join(card).join("device");
        std::fs::create_dir_all(&device).unwrap();
        std::fs::write(device.join("vendor"), format!("{vendor}\n")).unwrap();
        memory
            .into_iter()
            .for_each(|m| std::fs::write(device.join("mem_info_vram_total"), m.to_string()).unwrap());
    }

    #[test]
    fn an_opengl_card_of_unknown_kind_takes_the_kind_the_system_tells() {
        let root = tempfile::tempdir().unwrap();
        write(root.path(), "card0", "0x1002", Some(512 << 20));
        write(root.path(), "card1", "0x10de", None);
        write(root.path(), "card1-DP-1", "0x10de", None);
        write(root.path(), "renderD128", "0x1002", None);
        let found = cards(root.path());
        assert_eq!(found, [(AMD, DeviceType::IntegratedGpu), (NVIDIA, DeviceType::DiscreteGpu)]);
        assert_eq!(guess(AMD, &found), DeviceType::IntegratedGpu, "an AMD card with carved memory");
        assert_eq!(guess(NVIDIA, &found), DeviceType::DiscreteGpu);
        assert_eq!(guess(0, &found), DeviceType::Other, "an unknown vendor among two cards was guessed");
        assert_eq!(guess(0, &found[..1]), DeviceType::IntegratedGpu, "the only card of the computer");
        let big = tempfile::tempdir().unwrap();
        write(big.path(), "card0", "0x1002", Some(16 << 30));
        write(big.path(), "card1", "0x1002", Some(512 << 20));
        assert_eq!(guess(AMD, &cards(big.path())), DeviceType::Other, "two AMD cards of both kinds");
        write(big.path(), "card1", "0x1002", Some(16 << 30));
        assert_eq!(guess(AMD, &cards(big.path())), DeviceType::DiscreteGpu);
        assert!(cards(Path::new("/nonexistent")).is_empty());
        assert_eq!(guess(INTEL, &[]), DeviceType::Other);
    }

    fn linked(root: &Path, card: &str, address: &str) {
        let device = root.join("devices").join(address);
        std::fs::create_dir_all(&device).unwrap();
        std::fs::write(device.join("vendor"), "0x8086\n").unwrap();
        let class = root.join("class/drm").join(card);
        std::fs::create_dir_all(&class).unwrap();
        std::os::unix::fs::symlink(&device, class.join("device")).unwrap();
    }

    #[test]
    fn an_intel_card_on_its_own_bus_is_dedicated_and_the_one_on_bus_zero_integrated() {
        let root = tempfile::tempdir().unwrap();
        linked(root.path(), "card0", "0000:00:02.0");
        linked(root.path(), "card1", "0000:03:00.0");
        let found = cards(root.path());
        assert_eq!(found, [(INTEL, DeviceType::IntegratedGpu), (INTEL, DeviceType::DiscreteGpu)]);
        assert_eq!(guess(INTEL, &found), DeviceType::Other, "two Intel cards of both kinds were guessed");
        assert_eq!(guess(INTEL, &found[1..]), DeviceType::DiscreteGpu, "a dedicated Intel card");
        let plain = tempfile::tempdir().unwrap();
        write(plain.path(), "card0", "0x8086", None);
        assert_eq!(cards(plain.path()), [(INTEL, DeviceType::Other)], "an Intel card of unknown place");
    }
}
