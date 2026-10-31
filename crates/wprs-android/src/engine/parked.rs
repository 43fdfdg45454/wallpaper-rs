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
    static PARKED: RefCell<[Option<Parked>; 3]> = const { RefCell::new([None, None, None]) };
}

fn slot(preview: bool, lock: bool) -> usize {
    if preview { 2 } else { usize::from(lock) }
}

pub(crate) fn park(preview: bool, lock: bool, parked: Parked) {
    PARKED.with_borrow_mut(|p| p[slot(preview, lock)] = Some(parked));
}

pub(crate) fn take(preview: bool, lock: bool) -> Option<Parked> {
    PARKED.with_borrow_mut(|p| p[slot(preview, lock)].take())
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
        park(self.preview, self.lock, Parked { stage, image, raised, epoch });
    }
}
