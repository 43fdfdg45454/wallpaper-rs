use crate::Quality;
use serde::{Deserialize, Serialize};

const MODEST: [&str; 15] = [
    "hd graphics",
    "uhd graphics",
    "iris plus",
    "iris pro",
    "vega",
    "raven",
    "picasso",
    "renoir",
    "lucienne",
    "cezanne",
    "barcelo",
    "stoney",
    "carrizo",
    "powervr",
    "mali-t",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    T0,
    T1,
    T2,
    T3,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Gpu<'a> {
    pub name: &'a str,
    pub software: bool,
    pub discrete: bool,
    pub compute: bool,
    pub max_texture: u32,
    pub memory_mib: u64,
}

fn number_after(name: &str, key: &str) -> Option<u32> {
    let rest = name[name.find(key)? + key.len()..].chars().skip_while(|c| !c.is_ascii_digit());
    rest.take_while(char::is_ascii_digit).collect::<String>().parse().ok()
}

impl Tier {
    pub fn detect(gpu: &Gpu<'_>) -> Tier {
        let name = gpu.name.to_lowercase();
        let older = |key, newest| number_after(&name, key).is_some_and(|n| n < newest);
        let modest = MODEST.iter().any(|m| name.contains(m)) || older("mali-g", 715) || older("adreno", 700);
        let base = match () {
            _ if gpu.software || !gpu.compute => Tier::T0,
            _ if gpu.discrete => Tier::T3,
            _ if modest => Tier::T1,
            _ => Tier::T2,
        };
        base.min(Tier::cap(gpu))
    }

    pub(crate) fn cap(gpu: &Gpu<'_>) -> Tier {
        let below = |v: u64, limit: u64| v > 0 && v < limit;
        match () {
            _ if below(gpu.memory_mib, 2048) => Tier::T0,
            _ if below(gpu.memory_mib, 4096) || below(u64::from(gpu.max_texture), 8192) => Tier::T1,
            _ => Tier::T3,
        }
    }

    pub fn max_fps(self, on_battery: bool, wanted: u32, cost: Option<f32>) -> u32 {
        let wanted = wanted.clamp(1, 240);
        let fits = cost.is_some_and(|cost| cost * wanted as f32 <= self.budget() * 60.0);
        let cap = match (self, on_battery) {
            (Tier::T0, false) => 24,
            (Tier::T0, true) => 15,
            (Tier::T1, _) | (Tier::T2, true) => 30,
            (Tier::T2, false) if fits => wanted,
            (Tier::T2, false) | (Tier::T3, true) => 60,
            (Tier::T3, false) => wanted,
        };
        cap.min(wanted)
    }

    pub fn budget(self) -> f32 {
        [4.0, 8.0, 16.0, 40.0][self as usize]
    }

    pub fn ceiling(self) -> f32 {
        [2_000.0, 4_000.0, 8_000.0, 16_000.0][self as usize]
    }

    pub fn particles(self) -> u32 {
        [2_000, 10_000, 50_000, 200_000][self as usize]
    }

    pub fn overdraw(self) -> f32 {
        [1.0, 1.5, 3.0, 6.0][self as usize]
    }

    pub fn trail(self) -> u32 {
        [0, 4, 8, 16][self as usize]
    }

    pub fn quality(self, on_battery: bool) -> Quality {
        let base = [Quality::Q0, Quality::Q2, Quality::Q3, Quality::Q3][self as usize];
        if on_battery { base.lower() } else { base }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier(name: &str, discrete: bool) -> Tier {
        Tier::detect(&Gpu { name, discrete, compute: true, ..Gpu::default() })
    }

    #[test]
    fn older_integrated_gpus_land_on_the_middle_tier() {
        let modest = [
            "Intel(R) UHD Graphics 620 (KBL GT2)",
            "Intel(R) HD Graphics 520",
            "AMD Radeon Graphics (RADV RENOIR)",
            "AMD Radeon Vega 8 Graphics (RADV RAVEN)",
            "Mali-G57 MC2",
            "Mali-G710 MC10",
            "Adreno (TM) 650",
        ];
        assert_eq!(modest.map(|name| tier(name, false)), [Tier::T1; 7]);
        let newer = [
            "Intel(R) Iris(R) Xe Graphics",
            "AMD Radeon 780M (RADV PHOENIX)",
            "Mali-G715",
            "Adreno (TM) 740",
        ];
        assert_eq!(newer.map(|name| tier(name, false)), [Tier::T2; 4]);
        assert_eq!(tier("AMD Radeon RX 6600 (RADV NAVI23)", true), Tier::T3);
    }

    #[test]
    fn software_small_memory_and_no_compute_lower_the_tier() {
        let gpu = Gpu { name: "Intel(R) Iris(R) Xe Graphics", compute: true, ..Gpu::default() };
        let tiers = [
            Gpu { software: true, ..gpu },
            Gpu { compute: false, ..gpu },
            Gpu { memory_mib: 1536, ..gpu },
            Gpu { memory_mib: 3072, ..gpu },
            Gpu { max_texture: 4096, ..gpu },
            Gpu { memory_mib: 16_384, max_texture: 16_384, ..gpu },
        ]
        .map(|g| Tier::detect(&g));
        assert_eq!(tiers, [Tier::T0, Tier::T0, Tier::T0, Tier::T1, Tier::T1, Tier::T2]);
    }
}
