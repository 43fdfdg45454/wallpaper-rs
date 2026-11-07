use std::path::Path;
use wprs_core::GpuChoice;

fn pci(name: &str) -> bool {
    let b = name.as_bytes();
    let colons = [4, 7, 10];
    b.len() == 12 && b[4] == b':' && b[7] == b':' && b[10] == b'.' && {
        b.iter().enumerate().all(|(i, c)| colons.contains(&i) || c.is_ascii_hexdigit())
    }
}

fn parts(dev: u64) -> (u32, u32) {
    let major = ((dev >> 8) & 0xfff) as u32 | ((dev >> 32) & !0xfff) as u32;
    (major, (dev & 0xff) as u32 | ((dev >> 12) & !0xff) as u32)
}

pub(crate) fn bus_in(sys: &Path, dev: u64) -> Option<String> {
    let (major, minor) = parts(dev);
    let node = std::fs::canonicalize(sys.join(format!("dev/char/{major}:{minor}"))).ok()?;
    node.ancestors().filter_map(|p| p.file_name()?.to_str()).find(|n| pci(n)).map(str::to_lowercase)
}

pub fn bus_of(dev: u64) -> Option<String> {
    bus_in(Path::new("/sys"), dev)
}

pub(crate) fn guided(choice: &GpuChoice, bus: Option<&str>) -> Option<GpuChoice> {
    bus.filter(|_| choice.is_auto()).map(|b| GpuChoice::Bus(format!("{}{b}", GpuChoice::BUS)))
}
