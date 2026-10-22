use crate::draw::Target;
use crate::{RenderError, Renderer};
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

const GPU_TIMEOUT: Duration = Duration::from_secs(10);
const MAPPED: u8 = 1;
const FAILED: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetLimits {
    pub max_edge: u32,
    pub max_buffer_size: u64,
}

impl TargetLimits {
    pub fn check(&self, width: u32, height: u32) -> Result<(), RenderError> {
        let bytes = || u64::from(padded_row_bytes(width)) * u64::from(height);
        let valid = self.fits(width, height) && bytes() <= self.max_buffer_size;
        valid.then_some(()).ok_or(RenderError::InvalidSize(width, height))
    }

    pub(crate) fn fits(&self, width: u32, height: u32) -> bool {
        [width, height].iter().all(|edge| (1..=self.max_edge).contains(edge))
    }
}

impl Renderer {
    pub fn max_edge(&self) -> u32 {
        self.limits.max_edge
    }

    pub fn limits(&self) -> TargetLimits {
        self.limits
    }

    pub fn check_target(&self, width: u32, height: u32) -> Result<(), RenderError> {
        self.limits.check(width, height)
    }

    pub(crate) fn read(
        &self,
        target: &Target,
        dst: &mut Vec<u8>,
        done: wgpu::SubmissionIndex,
    ) -> Result<(), RenderError> {
        let (state, slice) = (target.mapped.clone(), target.readback.slice(..));
        state.store(0, Relaxed);
        slice.map_async(wgpu::MapMode::Read, move |r| {
            state.store(if r.is_ok() { MAPPED } else { FAILED }, Relaxed)
        });
        let wait = wgpu::PollType::Wait { submission_index: Some(done), timeout: Some(GPU_TIMEOUT) };
        self.device.poll(wait).map_err(|e| RenderError::DeviceLost(e.to_string()))?;
        if target.mapped.load(Relaxed) != MAPPED {
            return Err(RenderError::Readback("cannot map the readback buffer".into()));
        }
        let copied = slice.get_mapped_range().map(|view| copy_rows(&view, dst, target.size()));
        target.readback.unmap();
        copied.map_err(|e| RenderError::Readback(e.to_string()))
    }
}

fn copy_rows(src: &[u8], dst: &mut Vec<u8>, (width, height): (u32, u32)) {
    let row = width as usize * 4;
    dst.resize(row * height as usize, 0);
    let rows = src.chunks_exact(padded_row_bytes(width) as usize);
    dst.chunks_exact_mut(row).zip(rows).for_each(|(out, src)| out.copy_from_slice(&src[..row]));
}

pub(crate) fn padded_row_bytes(width: u32) -> u32 {
    (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
}
