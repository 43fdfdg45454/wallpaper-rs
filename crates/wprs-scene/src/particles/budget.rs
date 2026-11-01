use crate::{Quality, Tier};

pub const CPU_BOIDS: u32 = 2_000;
pub const MAX_BOIDS: u32 = 8_192;
pub const MAX_ROPE: u32 = 4_096;
pub const MAX_LONG: u32 = 64;

impl Tier {
    pub fn long_trail(self) -> u32 {
        [0, 16, 32, 64][self as usize]
    }

    pub fn neighbours(self) -> u32 {
        [8, 16, 32, 32][self as usize]
    }

    pub fn rope(self) -> u32 {
        [1024, 2048, 4096, 4096][self as usize]
    }
}

impl Quality {
    pub fn neighbours(self) -> u32 {
        [8, 16, 32, 32][self as usize]
    }

    pub fn long_trail(self, kept: u32) -> u32 {
        [0, kept / 4, kept / 2, kept][self as usize]
    }

    pub fn subdivide(self, wanted: u32) -> u32 {
        [0, wanted / 2, wanted, wanted][self as usize]
    }
}

pub fn cells(alloc: u32) -> u32 {
    (2 * alloc).next_power_of_two().clamp(64, 16_384)
}
