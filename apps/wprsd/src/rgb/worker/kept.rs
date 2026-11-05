use super::Session;
use crate::compositor::lock;
use std::time::Instant;
use wprs_core::rgb::Rgb;
use wprs_rgb::{Device, Error};

impl Session {
    pub(super) fn take(&mut self, wanted: &[&Device]) -> Result<(), Error> {
        let Some(client) = self.client.as_mut() else { return Ok(()) };
        let fresh: Vec<&Device> =
            wanted.iter().copied().filter(|d| !self.direct.contains(&d.index)).collect();
        for device in fresh {
            let mut status = lock(&self.status);
            let same = |d: &Device| d.index == device.index && d.name == device.name;
            if !status.saved.iter().any(same) {
                status.saved.push(device.clone());
            }
            drop(status);
            client.direct(device.index)?;
            self.direct.push(device.index);
        }
        Ok(())
    }

    pub(super) fn settle(&mut self, rgb: Rgb) {
        if rgb == self.rgb {
            return;
        }
        let placed = Rgb { places: self.rgb.places.clone(), ..rgb.clone() };
        if placed == self.rgb {
            self.restore();
            self.rgb = rgb;
            return self.repaint();
        }
        if !rgb.enabled {
            self.forget(None);
        }
        self.restore();
        (self.client, self.failed, self.rgb, self.opened) = (None, None, rgb, false);
        self.publish(None, Vec::new());
        self.retry();
    }

    pub(super) fn idle(&mut self) {
        if self.last.is_none() && self.test.is_none() {
            self.restore();
        }
    }

    pub(super) fn restore(&mut self) {
        self.direct.clear();
        let saved = std::mem::take(&mut lock(&self.status).saved);
        let Some(client) = self.client.as_mut().filter(|_| !saved.is_empty()) else { return };
        let devices = lock(&self.status).devices.clone();
        let present = |s: &&Device| devices.iter().any(|d| d.index == s.index && d.name == s.name);
        if let Err(e) = saved.iter().filter(present).try_for_each(|d| client.restore(d)) {
            (self.client, self.failed) = (None, Some(Instant::now()));
            self.publish(Some(e.to_string()), Vec::new());
        }
    }
}
