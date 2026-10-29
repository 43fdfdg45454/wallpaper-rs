use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
pub(super) use wprs_scene::Budget as Power;
use wprs_scene::Heat;

pub(super) const SYSFS: &str = "/sys/class";
const EVERY: Duration = Duration::from_secs(10);
const SENSORS: u32 = 16;
const LIMIT: i64 = 95_000;
const CHIPS: [&str; 8] = ["k10temp", "zenpower", "coretemp", "amdgpu", "radeon", "nouveau", "i915", "xe"];

fn watched(name: &str) -> bool {
    let name = name.trim();
    CHIPS.contains(&name) || name == "x86_pkg_temp" || ["cpu", "gpu", "soc"].iter().any(|k| name.contains(k))
}

fn read(path: PathBuf) -> Option<i64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn level(now: i64, limit: Option<i64>) -> Heat {
    match limit.filter(|l| *l > 0) {
        Some(l) if now >= l - 10_000 => Heat::Hot,
        Some(l) if now >= l - 20_000 => Heat::Warm,
        _ => Heat::Cool,
    }
}

fn entries(dir: PathBuf) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path())
}

pub(super) fn heat(root: &Path) -> Heat {
    let kind = |dir: &PathBuf, file| fs::read_to_string(dir.join(file)).is_ok_and(|n| watched(&n));
    let sensors = entries(root.join("hwmon")).filter(|dir| kind(dir, "name")).flat_map(|dir| {
        (1..=SENSORS).filter_map(move |n| {
            let now = read(dir.join(format!("temp{n}_input")))?;
            let limit =
                read(dir.join(format!("temp{n}_crit"))).or_else(|| read(dir.join(format!("temp{n}_max"))));
            Some(level(now, limit.or(Some(LIMIT))))
        })
    });
    let zones = entries(root.join("thermal")).filter(|dir| kind(dir, "type")).flat_map(|dir| {
        let now = read(dir.join("temp"));
        (0..SENSORS).filter_map(move |n| {
            let kind = fs::read_to_string(dir.join(format!("trip_point_{n}_type"))).ok()?;
            let limit = read(dir.join(format!("trip_point_{n}_temp")));
            match kind.trim() {
                "passive" if now? >= limit? => Some(Heat::Hot),
                "critical" | "hot" => Some(level(now?, limit)),
                _ => None,
            }
        })
    });
    sensors.chain(zones).max().unwrap_or_default()
}

impl super::state::State {
    pub(super) fn feel(&mut self, now: Instant) {
        let shown = self.outputs.values().any(|o| o.active && o.shown.scene.is_some());
        if !shown || self.felt.is_some_and(|at| now.duration_since(at) < EVERY) {
            return;
        }
        self.felt = Some(now);
        let felt = heat(Path::new(SYSFS));
        if std::mem::replace(&mut self.power.heat, felt) != felt {
            tracing::debug!(heat = ?felt, "the temperature changed the scene quality and rate");
            self.outputs.values_mut().filter(|o| o.shown.scene.is_some()).for_each(|o| o.dirty = true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn sensors_near_their_limits_warm_and_heat_the_scenes() {
        let root = std::env::temp_dir().join(format!("wprsd-heat-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        assert_eq!(heat(&root), Heat::Cool);
        write(&root, "hwmon/hwmon0/name", "amdgpu\n");
        write(&root, "hwmon/hwmon0/temp1_input", "61000\n");
        write(&root, "hwmon/hwmon0/temp1_crit", "100000\n");
        write(&root, "hwmon/hwmon1/name", "k10temp\n");
        write(&root, "hwmon/hwmon1/temp1_input", "70000\n");
        for (i, name) in ["nvme", "spd5118", "r8169"].iter().enumerate() {
            write(&root, &format!("hwmon/hwmon{}/name", i + 2), name);
            write(&root, &format!("hwmon/hwmon{}/temp1_input", i + 2), "84000\n");
            write(&root, &format!("hwmon/hwmon{}/temp1_crit", i + 2), "85000\n");
        }
        write(&root, "thermal/thermal_zone1/type", "acpitz\n");
        write(&root, "thermal/thermal_zone1/temp", "99000\n");
        write(&root, "thermal/thermal_zone1/trip_point_0_type", "passive\n");
        write(&root, "thermal/thermal_zone1/trip_point_0_temp", "90000\n");
        assert_eq!(heat(&root), Heat::Cool);
        write(&root, "hwmon/hwmon1/temp1_input", "76000\n");
        assert_eq!(heat(&root), Heat::Warm, "a CPU that reports no limit counts from 95 °C");
        write(&root, "hwmon/hwmon1/temp1_input", "70000\n");
        write(&root, "hwmon/hwmon0/temp2_input", "83000\n");
        write(&root, "hwmon/hwmon0/temp2_max", "100000\n");
        assert_eq!(heat(&root), Heat::Warm);
        write(&root, "thermal/thermal_zone0/type", "x86_pkg_temp\n");
        write(&root, "thermal/thermal_zone0/temp", "95000\n");
        write(&root, "thermal/thermal_zone0/trip_point_0_type", "passive\n");
        write(&root, "thermal/thermal_zone0/trip_point_0_temp", "95000\n");
        assert_eq!(heat(&root), Heat::Hot);
        fs::remove_dir_all(&root).unwrap();
    }
}
