pub const HALVES: f32 = 1.5;
const SLOWEST: u32 = 30;

pub fn halved(fps: u32) -> u32 {
    fps.div_ceil(2)
}

pub fn periods(halves: bool, [full, half]: [u32; 2]) -> [f32; 2] {
    let ms = |fps: u32| 1000.0 / fps.max(1) as f32;
    [ms(full), ms(if halves && half >= SLOWEST { half } else { full })]
}
