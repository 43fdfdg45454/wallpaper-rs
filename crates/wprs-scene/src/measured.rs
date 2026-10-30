use crate::{Gpu, Tier};
use serde::{Deserialize, Serialize};
use std::path::Path;

const METHOD: u32 = 1;
const STEPS: [(f64, Tier); 3] = [(0.2, Tier::T3), (0.5, Tier::T2), (2.0, Tier::T1)];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub adapter: String,
    pub driver: String,
    pub method: u32,
    pub unit_ms: f64,
    pub tier: Tier,
}

impl Tier {
    pub fn measured(unit_ms: f64) -> Tier {
        STEPS.iter().find(|(most, _)| unit_ms <= *most).map_or(Tier::T0, |(_, tier)| *tier)
    }

    pub fn calibrated(gpu: &Gpu<'_>, unit_ms: f64) -> Tier {
        let detected = Tier::detect(gpu);
        let up = [Tier::T1, Tier::T2, Tier::T3, Tier::T3][detected as usize];
        match detected {
            Tier::T0 => Tier::T0,
            _ => Tier::measured(unit_ms).min(up).min(Tier::cap(gpu)),
        }
    }
}

impl Calibration {
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
    measure: impl FnOnce() -> Option<f64>,
) -> (Calibration, bool) {
    let made = |unit_ms: f64| Calibration {
        adapter: adapter.into(),
        driver: driver.into(),
        method: METHOD,
        tier: if unit_ms > 0.0 { Tier::calibrated(gpu, unit_ms) } else { Tier::detect(gpu) },
        unit_ms,
    };
    if Tier::detect(gpu) == Tier::T0 {
        return (made(0.0), false);
    }
    let read = cache.and_then(|p| std::fs::read(p).ok());
    let kept = read.and_then(|bytes| serde_json::from_slice::<Calibration>(&bytes).ok());
    if let Some(kept) = kept.filter(|c| c.measured() && c.same(&made(0.0))) {
        return (made(kept.unit_ms), false);
    }
    let found = made(measure().filter(|ms| ms.is_finite() && *ms > 0.0).unwrap_or(0.0));
    let fresh = found.measured();
    (found, fresh)
}
