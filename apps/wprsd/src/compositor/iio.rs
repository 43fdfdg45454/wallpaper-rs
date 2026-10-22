use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use wprs_core::motion::{Tilt, gravity, mount_matrix};

const STRAIGHT: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
const XYZ: [&str; 3] = ["x", "y", "z"];
const SENSE: Duration = Duration::from_millis(33);
const RESCAN: Duration = Duration::from_secs(5);
const DEVICES: &str = "/sys/bus/iio/devices";

pub(super) struct Accelerometer {
    dir: PathBuf,
    scale: f32,
    offset: [f32; 3],
    matrix: [[f32; 3]; 3],
}

fn number(path: PathBuf) -> Option<f32> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

impl Accelerometer {
    pub(super) fn find(root: &Path) -> Option<Self> {
        let mut dirs: Vec<PathBuf> = std::fs::read_dir(root).ok()?.flatten().map(|e| e.path()).collect();
        dirs.sort();
        let dir = dirs.into_iter().find(|d| d.join("in_accel_x_raw").is_file())?;
        let value = |name: &str| number(dir.join(format!("in_accel_{name}")));
        let scale = value("scale").or_else(|| value("x_scale")).unwrap_or(1.0);
        let offset =
            XYZ.map(|axis| value(&format!("{axis}_offset")).or_else(|| value("offset")).unwrap_or(0.0));
        let matrix = ["in_accel_mount_matrix", "mount_matrix"]
            .iter()
            .find_map(|name| mount_matrix(&std::fs::read_to_string(dir.join(name)).ok()?));
        tracing::debug!("following an accelerometer for the parallax");
        Some(Self { scale, offset, matrix: matrix.unwrap_or(STRAIGHT), dir })
    }

    pub(super) fn gravity(&self) -> Option<[f32; 3]> {
        let [x, y, z] = XYZ.map(|axis| number(self.dir.join(format!("in_accel_{axis}_raw"))));
        let raw = [x?, y?, z?];
        Some(gravity(std::array::from_fn(|i| raw[i] + self.offset[i]), self.scale, &self.matrix))
    }
}

#[derive(Default)]
pub(super) struct Sensor {
    device: Option<Accelerometer>,
    scanned: Option<Instant>,
    sensed: Option<Instant>,
    tilt: Tilt,
    goal: [f32; 2],
}

impl Sensor {
    pub(super) fn goal(&mut self, now: Instant, root: Option<&Path>) -> [f32; 2] {
        if self.sensed.is_some_and(|at| now < at + SENSE) {
            return self.goal;
        }
        let dt = self.sensed.replace(now).map_or(0.0, |at| (now - at).as_secs_f32().min(1.0));
        if self.device.is_none() && self.scanned.is_none_or(|at| now >= at + RESCAN) {
            self.scanned = Some(now);
            self.device = Accelerometer::find(root.unwrap_or(Path::new(DEVICES)));
        }
        match self.device.as_ref().map(Accelerometer::gravity) {
            Some(Some(g)) => self.goal = self.tilt.follow(g, dt),
            Some(None) => (self.device, self.tilt, self.goal) = (None, Tilt::default(), [0.0; 2]),
            None => self.goal = [0.0; 2],
        }
        self.goal
    }

    pub(super) fn present(&self) -> bool {
        self.device.is_some()
    }

    pub(super) fn rescan(&self) -> Option<Instant> {
        self.scanned.filter(|_| self.device.is_none()).map(|at| at + RESCAN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, values: &[(&str, &str)]) {
        std::fs::create_dir_all(dir).unwrap();
        values.iter().for_each(|(name, value)| std::fs::write(dir.join(name), format!("{value}\n")).unwrap());
    }

    #[test]
    fn sysfs_accelerometers_are_found_scaled_offset_and_turned() {
        let root = tempfile::tempdir().unwrap();
        assert!(Accelerometer::find(root.path()).is_none());
        write(&root.path().join("iio:device0"), &[("in_illuminance_raw", "40")]);
        let accel = root.path().join("iio:device1");
        let raw = [("in_accel_x_raw", "-200"), ("in_accel_y_raw", "-20"), ("in_accel_z_raw", "981")];
        write(&accel, &raw);
        write(&accel, &[("in_accel_scale", "0.010000"), ("in_accel_y_offset", "20")]);
        write(&accel, &[("in_accel_mount_matrix", "0, 1, 0; -1, 0, 0; 0, 0, 1")]);
        let found = Accelerometer::find(root.path()).unwrap();
        let g = found.gravity().unwrap();
        assert!(g.iter().zip([0.0, 2.0, 9.81]).all(|(a, b)| (a - b).abs() < 1e-3), "{g:?}");
        std::fs::write(accel.join("in_accel_y_raw"), "busy").unwrap();
        assert!(found.gravity().is_none());
    }

    #[test]
    fn a_sensor_that_goes_away_is_looked_for_again() {
        let root = tempfile::tempdir().unwrap();
        let (mut sensor, start) = (Sensor::default(), Instant::now());
        assert_eq!(sensor.goal(start, Some(root.path())), [0.0; 2]);
        assert_eq!((sensor.present(), sensor.rescan()), (false, Some(start + RESCAN)));
        let accel = root.path().join("iio:device0");
        write(&accel, &[("in_accel_x_raw", "0"), ("in_accel_y_raw", "0"), ("in_accel_z_raw", "981")]);
        sensor.goal(start + SENSE, Some(root.path()));
        assert!(!sensor.present(), "rescans wait");
        sensor.goal(start + RESCAN, Some(root.path()));
        assert!(sensor.present() && sensor.rescan().is_none());
        std::fs::remove_dir_all(&accel).unwrap();
        sensor.goal(start + RESCAN + SENSE, Some(root.path()));
        assert!(!sensor.present());
        sensor.goal(start + RESCAN * 2 + SENSE, Some(root.path()));
        assert!(!sensor.present());
    }
}
