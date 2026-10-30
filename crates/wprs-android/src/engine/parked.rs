use super::Engine;
use crate::gpu::Key;
use crate::relief::Raised;
use crate::stage::Stage;
use std::cell::RefCell;
use std::sync::Arc;
use wprs_render::GpuImage;

pub(crate) struct Parked {
    pub(crate) stage: Option<Stage>,
    pub(crate) image: Option<(Key, Arc<GpuImage>)>,
    pub(crate) raised: Option<Raised>,
    pub(crate) epoch: u64,
}

thread_local! {
    static PARKED: RefCell<[Option<Parked>; 2]> = const { RefCell::new([None, None]) };
}

pub(crate) fn park(preview: bool, parked: Parked) {
    PARKED.with_borrow_mut(|p| p[usize::from(preview)] = Some(parked));
}

pub(crate) fn take(preview: bool) -> Option<Parked> {
    PARKED.with_borrow_mut(|p| p[usize::from(preview)].take())
}

pub(crate) fn forget() {
    let gone = PARKED.with_borrow_mut(std::mem::take);
    drop(gone);
}

impl Drop for Engine {
    fn drop(&mut self) {
        let mut stage = self.stage.take();
        stage.iter_mut().for_each(Stage::hide);
        let epoch = self.target.as_ref().map_or(self.checked.0, |(gpu, _)| gpu.epoch);
        let (image, raised) = (self.image.take(), self.raised.take());
        park(self.preview, Parked { stage, image, raised, epoch });
    }
}
