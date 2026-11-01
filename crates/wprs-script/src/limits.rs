use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    pub memory: usize,
    pub stack: usize,
    pub update: Duration,
    pub load: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        let (update, load) = (Duration::from_millis(4), Duration::from_millis(100));
        Limits { memory: 32 << 20, stack: 256 << 10, update, load }
    }
}
