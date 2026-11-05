use super::SceneState;
use crate::media::Frame;
use std::sync::Arc;
use std::time::Instant;

impl SceneState {
    pub(super) fn hear_media(&mut self, frame: Option<Arc<Frame>>) -> bool {
        let serial = frame.as_ref().map_or(0, |f| f.serial);
        if std::mem::replace(&mut self.media, serial) == serial {
            return false;
        }
        let state = frame.map(|f| f.at(Instant::now())).unwrap_or_default();
        self.runtime.set_media(state)
    }
}
