use crate::{Gpu, Tier};
use serde::{Deserialize, Serialize};
use std::path::Path;

const RUNS: u32 = 3;
const STEPS: [(f64, Tier); 3] = [(0.2, Tier::T3), (0.5, Tier::T2), (2.0, Tier::T1)];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub adapter: String,
    pub driver: String,
    pub method: u32,
    pub unit_ms: f64,
    pub tier: Tier,
    #[serde(default)]
    pub runs: u32,
}

impl Tier {
    pub fn measured(unit_ms: f64) -> Tier {
        STEPS.iter().find(|(most, _)| unit_ms <= *most).map_or(Tier::T0, |(_, tier)| *tier)
    }

    pub fn calibrated(gpu: &Gpu<'_>, unit_ms: f64) -> Tier {
        let detected = Tier::detect(gpu);
        let up = [Tier::T1, Tier::T2, Tier::T3, Tier::T3][detected as usize];
        let down = [Tier::T0, Tier::T0, Tier::T1, Tier::T2][detected as usize];
        match detected {
            Tier::T0 => Tier::T0,
            _ => Tier::measured(unit_ms).clamp(down, up).min(Tier::cap(gpu)),
        }
    }
}

impl Calibration {
    pub const FENCE: u32 = 1;
    pub const STAMPS: u32 = 2;

    pub fn stamped(&self) -> bool {
        self.method == Calibration::STAMPS && self.measured()
    }

    pub fn measured(&self) -> bool {
        self.unit_ms > 0.0
    }

    pub fn keep(&self, path: &Path) -> std::io::Result<()> {
        path.parent().map_or(Ok(()), std::fs::create_dir_all)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, path)
    }

    pub fn same(&self, other: &Calibration) -> bool {
        (&self.adapter, &self.driver, self.method) == (&other.adapter, &other.driver, other.method)
    }
}

pub fn calibrate(
    cache: Option<&Path>,
    (adapter, driver): (&str, &str),
    gpu: &Gpu<'_>,
    method: u32,
    measure: impl FnOnce() -> Option<f64>,
) -> (Calibration, bool) {
    let made = |unit_ms: f64, runs: u32| Calibration {
        adapter: adapter.into(),
        driver: driver.into(),
        method,
        tier: if unit_ms > 0.0 { Tier::calibrated(gpu, unit_ms) } else { Tier::detect(gpu) },
        unit_ms,
        runs,
    };
    if Tier::detect(gpu) == Tier::T0 {
        return (made(0.0, 0), false);
    }
    let read = cache.and_then(|p| std::fs::read(p).ok());
    let kept = read.and_then(|bytes| serde_json::from_slice::<Calibration>(&bytes).ok());
    let kept = kept.filter(|c| c.measured() && c.same(&made(0.0, 0)));
    let low = |c: &Calibration| Tier::calibrated(gpu, c.unit_ms) < Tier::detect(gpu) && c.runs.max(1) < RUNS;
    if let Some(kept) = kept.as_ref().filter(|c| !low(c)) {
        return (made(kept.unit_ms, kept.runs), false);
    }
    let measured = measure().filter(|ms| ms.is_finite() && *ms > 0.0);
    let (best, runs) = match (&kept, measured) {
        (Some(k), Some(ms)) => (ms.min(k.unit_ms), k.runs.max(1) + 1),
        (Some(k), None) => return (made(k.unit_ms, k.runs), false),
        (None, ms) => (ms.unwrap_or(0.0), u32::from(ms.is_some())),
    };
    let found = made(best, runs);
    let fresh = found.measured();
    (found, fresh)
}
