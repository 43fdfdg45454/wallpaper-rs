use super::*;

fn write(root: &Path, card: &str, vendor: &str, memory: Option<u64>) {
    let device = root.join("class/drm").join(card).join("device");
    std::fs::create_dir_all(&device).unwrap();
    std::fs::write(device.join("vendor"), format!("{vendor}\n")).unwrap();
    memory
        .into_iter()
        .for_each(|m| std::fs::write(device.join("mem_info_vram_total"), m.to_string()).unwrap());
}

fn boot(root: &Path, card: &str) {
    std::fs::write(root.join("class/drm").join(card).join("device/boot_vga"), "1\n").unwrap();
}

fn kinds(found: &[Card]) -> Vec<(u32, DeviceType)> {
    found.iter().map(|c| (c.vendor, c.kind)).collect()
}

#[test]
fn an_opengl_card_of_unknown_kind_takes_the_kind_the_system_tells() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "card0", "0x1002", Some(512 << 20));
    write(root.path(), "card1", "0x10de", None);
    write(root.path(), "card1-DP-1", "0x10de", None);
    write(root.path(), "renderD128", "0x1002", None);
    let found = cards(root.path());
    assert_eq!(kinds(&found), [(AMD, DeviceType::IntegratedGpu), (NVIDIA, DeviceType::DiscreteGpu)]);
    assert_eq!(guess(AMD, &found, false), DeviceType::IntegratedGpu, "an AMD card with carved memory");
    assert_eq!(guess(NVIDIA, &found, false), DeviceType::DiscreteGpu);
    assert_eq!(guess(0, &found, false), DeviceType::Other, "an unknown vendor among two cards was guessed");
    assert_eq!(guess(0, &found[..1], false), DeviceType::IntegratedGpu, "the only card of the computer");
    let big = tempfile::tempdir().unwrap();
    write(big.path(), "card0", "0x1002", Some(16 << 30));
    write(big.path(), "card1", "0x1002", Some(512 << 20));
    assert_eq!(guess(AMD, &cards(big.path()), false), DeviceType::Other, "two AMD cards of both kinds");
    write(big.path(), "card1", "0x1002", Some(16 << 30));
    assert_eq!(guess(AMD, &cards(big.path()), false), DeviceType::DiscreteGpu);
    assert!(cards(Path::new("/nonexistent")).is_empty());
    assert_eq!(guess(INTEL, &[], false), DeviceType::Other);
}

#[test]
fn cards_of_one_vendor_and_both_kinds_take_the_kind_of_the_card_opengl_uses() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "card0", "0x1002", Some(16 << 30));
    write(root.path(), "card1", "0x1002", Some(512 << 20));
    boot(root.path(), "card1");
    let found = cards(root.path());
    assert_eq!(found.iter().map(|c| c.boot).collect::<Vec<_>>(), [false, true]);
    assert_eq!(guess(AMD, &found, false), DeviceType::IntegratedGpu, "the card that starts the screen");
    assert_eq!(guess(AMD, &found, true), DeviceType::DiscreteGpu, "the other card asked with DRI_PRIME");
    write(root.path(), "card2", "0x1002", Some(512 << 20));
    assert_eq!(
        guess(AMD, &cards(root.path()), true),
        DeviceType::Other,
        "other cards of both kinds were guessed"
    );
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
    assert_eq!(kinds(&found), [(INTEL, DeviceType::IntegratedGpu), (INTEL, DeviceType::DiscreteGpu)]);
    assert_eq!(guess(INTEL, &found, false), DeviceType::Other, "two Intel cards of both kinds were guessed");
    assert_eq!(guess(INTEL, &found[1..], false), DeviceType::DiscreteGpu, "a dedicated Intel card");
    let plain = tempfile::tempdir().unwrap();
    write(plain.path(), "card0", "0x8086", None);
    assert_eq!(kinds(&cards(plain.path())), [(INTEL, DeviceType::Other)], "an Intel card of unknown place");
}

#[test]
fn an_intel_card_without_a_pci_address_is_told_by_its_device_id() {
    let root = tempfile::tempdir().unwrap();
    for (card, id) in [("card0", "0x9a49"), ("card1", "0x56a0"), ("card2", "0xe20b"), ("card3", "0x4905")] {
        write(root.path(), card, "0x8086", None);
        std::fs::write(root.path().join("class/drm").join(card).join("device/device"), id).unwrap();
    }
    let found = kinds(&cards(root.path()));
    let want = [
        DeviceType::IntegratedGpu,
        DeviceType::DiscreteGpu,
        DeviceType::DiscreteGpu,
        DeviceType::DiscreteGpu,
    ];
    assert_eq!(found.iter().map(|(_, k)| *k).collect::<Vec<_>>(), want);
}
