use basisu::TargetFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    Bc7,
    Astc,
    Etc2,
    Rgba,
}

pub const TARGETS: [Target; 4] = [Target::Bc7, Target::Astc, Target::Etc2, Target::Rgba];

impl Target {
    pub fn name(self) -> &'static str {
        match self {
            Target::Bc7 => "bc7",
            Target::Astc => "astc",
            Target::Etc2 => "etc2",
            Target::Rgba => "rgba",
        }
    }

    pub fn parse(name: &str) -> Option<Target> {
        TARGETS.into_iter().find(|t| t.name() == name)
    }

    pub fn class(self) -> &'static str {
        match self {
            Target::Bc7 => "desktop",
            Target::Astc => "phone",
            Target::Etc2 => "older phone",
            Target::Rgba => "any other",
        }
    }

    pub(crate) fn basis(self) -> TargetFormat {
        match self {
            Target::Bc7 => TargetFormat::Bc7Rgba,
            Target::Astc => TargetFormat::Astc4x4Rgba,
            Target::Etc2 => TargetFormat::Etc2Rgba,
            Target::Rgba => TargetFormat::Rgba32,
        }
    }

    pub fn bytes(self, (w, h): (u32, u32)) -> u64 {
        let blocks = u64::from(w.div_ceil(4)) * u64::from(h.div_ceil(4));
        match self {
            Target::Rgba => u64::from(w) * u64::from(h) * 4,
            _ => blocks * 16,
        }
    }

    pub fn compressed(self) -> bool {
        self != Target::Rgba
    }
}
