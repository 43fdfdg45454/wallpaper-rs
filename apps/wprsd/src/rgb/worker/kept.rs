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
            let same = |d: &Device| d.index == device.index && d.key == device.key;
            if !status.saved.iter().any(same) {
                status.saved.push(device.clone());
                self.disk.iter().for_each(|path| super::disk::save(path, &status.saved));
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

    pub(super) fn stop(&mut self) {
        self.test = None;
        lock(&self.status).testing = false;
        self.forget(None);
    }

    pub(super) fn restore(&mut self) {
        self.direct.clear();
        let saved = std::mem::take(&mut lock(&self.status).saved);
        self.owed = !saved.is_empty();
        let Some(client) = self.client.as_mut() else { return lock(&self.status).saved = saved };
        let devices = lock(&self.status).devices.clone();
        let same = |a: &Device, b: &Device| a.index == b.index && a.key == b.key;
        let (mut left, mut failed, before) = (Vec::new(), None, saved.len());
        for device in saved {
            if failed.is_some() || !devices.iter().any(|d| same(d, &device)) {
                left.push(device);
                continue;
            }
            match client.restore(&device) {
                Ok(()) => {
                    let mut status = lock(&self.status);
                    if let Some(shown) = status.devices.iter_mut().find(|d| same(d, &device)) {
                        (shown.mode, shown.colors) = (device.mode, device.colors);
                    }
                }
                Err(e) => {
                    left.push(device);
                    failed = Some(e);
                }
            }
        }
        let mut status = lock(&self.status);
        if let Some(path) = self.disk.as_ref().filter(|_| before != left.len()) {
            super::disk::save(path, &left);
        }
        self.owed = !left.is_empty();
        status.saved = left;
        drop(status);
        if let Some(e) = failed {
            (self.client, self.failed) = (None, Some(Instant::now()));
            self.publish(Some(e.to_string()), Vec::new());
        }
    }
}
