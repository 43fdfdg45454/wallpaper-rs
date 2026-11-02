use super::pipes::Key;
use wprs_scene::Frame;

#[derive(Clone, Copy)]
pub(crate) struct Call {
    pub(super) model: u64,
    pub(super) first: u32,
    pub(super) count: u32,
    pub(super) maps: usize,
    pub(super) object: u32,
    pub(super) finish: u32,
    pub(super) key: Key,
    pub(super) far: f32,
}

pub(super) fn sized(size: [f32; 2], scale: f32, frame: &Frame, edge: u32) -> (u32, u32) {
    let k = scale * frame.quality.render_scale();
    let [w, h] = size.map(|v| ((v * k).round() as u32).clamp(1, edge));
    (w, h)
}

pub(super) fn put<T: bytemuck::Pod>(bytes: &mut Vec<u8>, at: u64, value: &T) {
    let at = at as usize;
    let raw = bytemuck::bytes_of(value);
    if bytes.len() < at + raw.len() {
        bytes.resize(at + raw.len(), 0);
    }
    bytes[at..at + raw.len()].copy_from_slice(raw);
}
