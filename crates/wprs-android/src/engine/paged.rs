use super::Engine;
use crate::power::Power;
use crate::{choice, gpu, page};

impl Engine {
    pub(crate) fn page(&mut self, handle: i64) -> Option<String> {
        let shown = choice::shown_on(self.preview, self.lock).filter(|shown| shown.web);
        let spec = shown.map(|s| page::spec(handle, &s.uid, &s.path, self.preview));
        let spec = spec.and_then(|spec| spec.inspect_err(|e| log::warn!("cannot show the page: {e}")).ok());
        let Some(spec) = spec.filter(|s| !s.is_null()) else {
            if self.paged.take().is_some() {
                page::close(handle);
                (self.checked, self.dirty) = ((0, 0, (0, 0)), true);
            }
            return None;
        };
        if self.paged.replace(handle).is_none() {
            self.hint(0);
            (self.target, self.image, self.raised, self.stage, self.fading) = (None, None, None, None, None);
            gpu::trim();
        }
        Some(spec.to_string())
    }

    pub(super) fn page_inputs(&self, handle: i64) -> i32 {
        let hears = page::hearing(handle) && Power::now().moving() && crate::power::hears();
        i32::from(hears) | (i32::from(page::touched(handle)) << 1)
    }
}
