use basisu::TargetFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    Bc7,
    Astc,
    Etc2,
    Rgba,
    Bc6h,
    AstcHdr,
    Half,
}

pub const TARGETS: [Target; 4] = [Target::Bc7, Target::Astc, Target::Etc2, Target::Rgba];
pub const HDR_TARGETS: [Target; 3] = [Target::Bc6h, Target::AstcHdr, Target::Half];

impl Target {
    pub fn name(self) -> &'static str {
        match self {
            Target::Bc7 => "bc7",
            Target::Astc => "astc",
            Target::Etc2 => "etc2",
            Target::Rgba => "rgba",
            Target::Bc6h => "bc6h",
            Target::AstcHdr => "astc-hdr",
            Target::Half => "half",
        }
    }

    pub fn parse(name: &str) -> Option<Target> {
        TARGETS.into_iter().chain(HDR_TARGETS).find(|t| t.name() == name)
    }

    pub fn class(self) -> &'static str {
        match self {
            Target::Bc7 | Target::Bc6h => "desktop",
            Target::Astc | Target::AstcHdr => "phone",
            Target::Etc2 => "older phone",
            Target::Rgba | Target::Half => "any other",
        }
    }

    pub fn hdr(self) -> bool {
        HDR_TARGETS.contains(&self)
    }

    pub fn lit(self) -> Target {
        match self {
            Target::Bc7 | Target::Bc6h => Target::Bc6h,
            Target::Astc | Target::AstcHdr => Target::AstcHdr,
            _ => Target::Half,
        }
    }

    pub(crate) fn basis(self) -> TargetFormat {
        match self {
            Target::Bc7 => TargetFormat::Bc7Rgba,
            Target::Astc => TargetFormat::Astc4x4Rgba,
            Target::Etc2 => TargetFormat::Etc2Rgba,
            Target::Rgba => TargetFormat::Rgba32,
            Target::Bc6h => TargetFormat::Bc6h,
            Target::AstcHdr => TargetFormat::AstcHdr4x4Rgba,
            Target::Half => TargetFormat::RgbaHalf,
        }
    }

    pub(crate) fn unit(self) -> (u32, usize) {
        match self {
            Target::Rgba => (1, 4),
            Target::Half => (1, 8),
            _ => (4, 16),
        }
    }

    pub fn bytes(self, (w, h): (u32, u32)) -> u64 {
        let (side, bytes) = self.unit();
        u64::from(w.div_ceil(side)) * u64::from(h.div_ceil(side)) * bytes as u64
    }

    pub fn compressed(self) -> bool {
        self.unit().0 > 1
    }
}
