use super::PlayerSlot;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use wprs_render::{DeepPlanes, RenderError, Renderer, Shared, log_allowed};
use wprs_video::{Frame, Transfer};

impl PlayerSlot {
    pub(super) fn upload(&mut self, path: &Path, renderer: &Renderer, frame: Frame, now: Instant) -> bool {
        let (data, w, h, stride) = (frame.data(), frame.width, frame.height, frame.stride);
        let uploaded = match (frame.dmabuf(), &mut self.texture) {
            _ if frame.hdr.is_some() => self.deep(renderer, &frame),
            (Some(shared), _) => {
                let (modifier, bt709, full, turn) = (shared.modifier, shared.bt709, shared.full, shared.turn);
                let planes = shared.planes();
                let wanted = Shared { size: (w, h), modifier, planes, bt709, full, turn, fit: self.max_size };
                let image = renderer.convert_shared(&mut self.imported, &wanted).cloned();
                image.map(|image| drop(self.texture.replace(image)))
            }
            (None, Some(texture)) if self.decode == "copy" => {
                renderer.update_rgba(texture, data, w, h, stride)
            }
            (None, _) => renderer.upload_rgba(data, w, h, stride).map(|t| drop(self.texture.replace(t))),
        };
        let shared = frame.dmabuf().is_some();
        if uploaded.is_ok() {
            self.imported = self.imported.take().filter(|_| shared);
            (self.decode, self.held) = (if shared { "zero-copy" } else { "copy" }, shared.then_some(frame));
        } else if shared {
            self.copy_instead(now);
        }
        let Err(e) = uploaded else {
            (self.error, self.failures) = (None, 0);
            self.due = self.due.filter(|_| self.player.is_none());
            return true;
        };
        static UPLOAD_ERRORS: AtomicU64 = AtomicU64::new(0);
        if log_allowed(&UPLOAD_ERRORS) {
            tracing::warn!("cannot upload a video frame: {e}");
            tracing::debug!(path = %path.display(), "video with frames that cannot be uploaded");
        }
        false
    }

    fn deep(&mut self, renderer: &Renderer, frame: &Frame) -> Result<(), RenderError> {
        let (Some(hdr), Some(planes)) = (frame.hdr, frame.planes()) else {
            return Err(RenderError::InvalidSize(0, 0));
        };
        let wanted = DeepPlanes {
            size: (frame.width, frame.height),
            planes,
            hlg: hdr.transfer == Transfer::Hlg,
            full: hdr.full,
        };
        let image = renderer.convert_deep(&mut self.deep, &wanted)?.clone();
        self.texture = Some(image);
        Ok(())
    }
}
