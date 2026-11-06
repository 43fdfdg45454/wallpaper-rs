use super::{Ask, Status};
use crate::compositor::lock;
use placed::{area, zone};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wprs_core::rgb::Place;
use wprs_core::rgb::Rgb;
use wprs_rgb::{Client, Device, Strip, colors, pattern};

const WAIT: Duration = Duration::from_secs(2);
const RETRY: Duration = Duration::from_secs(10);
const TEST: Duration = Duration::from_secs(3);
const STEP: Duration = Duration::from_millis(33);
const LISTEN: Duration = Duration::from_secs(1);
const IDLE: Duration = Duration::from_secs(3600);

#[derive(Default)]
struct Session {
    status: Arc<Mutex<Status>>,
    rgb: Rgb,
    client: Option<Client>,
    failed: Option<Instant>,
    last: Option<(String, Strip)>,
    test: Option<(Instant, Option<u32>)>,
    opened: bool,
    direct: Vec<u32>,
    disk: Option<PathBuf>,
    owed: bool,
}

pub(super) fn spawn(asks: Receiver<Ask>, status: Arc<Mutex<Status>>, disk: Option<PathBuf>) {
    lock(&status).saved = disk.as_deref().map(disk::load).unwrap_or_default();
    let owed = !lock(&status).saved.is_empty();
    let session = Session { status, disk, owed, ..Session::default() };
    let started = std::thread::Builder::new().name("wprsd-rgb".into()).spawn(move || run(asks, session));
    if let Err(e) = started {
        tracing::warn!("cannot start the RGB lighting: {e}");
    }
}

fn run(asks: Receiver<Ask>, mut s: Session) {
    loop {
        lock(&s.status).held = s.held();
        match asks.recv_timeout(s.wait()) {
            Ok(Ask::Settings(rgb)) => s.settle(rgb),
            Ok(Ask::Forget(source)) => s.forget(Some(&source)),
            Ok(Ask::Covered) => s.retry(),
            Ok(Ask::Frame(source, strip)) => {
                lock(&s.status).source = Some(source.clone());
                s.last = Some((source.clone(), strip.clone()));
                if s.test.is_none() && !s.unwanted(Some(&source)) {
                    s.paint(&strip, None);
                }
            }
            Ok(Ask::Connect(reply)) => {
                (s.client, s.failed, s.opened) = (None, None, false);
                s.direct.clear();
                reply.send(s.ready()).ok();
            }
            Ok(Ask::Test(device, reply)) => {
                let known =
                    |s: &Session| device.is_none_or(|d| lock(&s.status).devices.iter().any(|v| v.index == d));
                let ready = s.ready().and_then(|()| match known(&s) {
                    true => Ok(()),
                    false => Err(format!("there is no RGB device {}", device.unwrap_or_default())),
                });
                if ready.is_ok() {
                    (s.test, s.opened) = (Some((Instant::now(), device)), false);
                    lock(&s.status).testing = true;
                }
                reply.send(ready).ok();
            }
            Ok(Ask::Stop(reply)) => {
                s.stop();
                reply.send(()).ok();
            }
            Err(RecvTimeoutError::Timeout) => s.tick(),
            Err(RecvTimeoutError::Disconnected) => return s.stop(),
        }
    }
}

impl Session {
    fn forget(&mut self, source: Option<&str>) {
        if self.last.as_ref().is_some_and(|(from, _)| source.is_none_or(|s| s == from)) {
            self.last = None;
            lock(&self.status).source = None;
        }
        self.idle();
    }

    fn repaint(&mut self) {
        match self.last.clone() {
            Some((source, strip)) if super::allowed(&source) => self.paint(&strip, None),
            Some(_) => self.forget(None),
            None => {}
        }
    }

    fn publish(&mut self, error: Option<String>, devices: Vec<Device>) {
        let connected = self.client.is_some();
        if lock(&self.status).show(connected, devices, error) {
            super::wake();
        }
    }

    fn ready(&mut self) -> Result<(), String> {
        if !self.rgb.enabled {
            return Err("RGB lighting is off; turn it on in the settings first".into());
        }
        if self.client.is_some() {
            return Ok(());
        }
        if self.failed.is_some_and(|at| at.elapsed() < RETRY) {
            return Err(lock(&self.status).error.clone().unwrap_or_default());
        }
        let opened = Client::connect(&self.rgb.server, WAIT).and_then(|mut c| Ok((c.devices()?, c)));
        match opened {
            Ok((devices, client)) => {
                (self.client, self.direct) = (Some(client), Vec::new());
                self.publish(None, devices);
                if self.owed && self.last.is_none() && self.test.is_none() {
                    self.restore();
                }
                self.client
                    .as_ref()
                    .map(drop)
                    .ok_or_else(|| lock(&self.status).error.clone().unwrap_or_default())
            }
            Err(e) => {
                (self.client, self.failed) = (None, Some(Instant::now()));
                self.publish(Some(e.to_string()), Vec::new());
                Err(e.to_string())
            }
        }
    }

    fn paint(&mut self, strip: &Strip, only: Option<u32>) {
        if self.ready().is_err() {
            return;
        }
        let devices = lock(&self.status).devices.clone();
        let off = |d: &Device| only.is_none() && self.rgb.place(&d.key) == Some(Place::Off);
        let wanted: Vec<&Device> =
            devices.iter().filter(|d| only.is_none_or(|i| i == d.index) && !off(d)).collect();
        let taken = self.take(&wanted);
        let Some(client) = self.client.as_mut() else { return };
        let rgb = &self.rgb;
        let painted = taken.and_then(|()| {
            wanted.iter().try_for_each(|d| {
                client.paint(d.index, &colors(d, strip, area(rgb, d), &|z| zone(rgb, d, z)))
            })
        });
        match painted {
            Ok(()) => lock(&self.status).frames += 1,
            Err(e) => {
                (self.client, self.failed) = (None, Some(Instant::now()));
                self.publish(Some(e.to_string()), Vec::new());
            }
        }
    }

    fn tick(&mut self) {
        if let Some((started, device)) = self.test {
            if started.elapsed() < TEST {
                return self.paint(&pattern(started.elapsed().as_secs_f32() / 2.0), device);
            }
            self.test = None;
            lock(&self.status).testing = false;
            self.repaint();
            return self.idle();
        }
        let changed = self.client.as_mut().map(Client::heard);
        match changed {
            Some(Ok(true)) => {
                self.client = None;
                if self.ready().is_ok() {
                    self.repaint();
                }
            }
            Some(Err(e)) => {
                (self.client, self.failed) = (None, Some(Instant::now()));
                self.publish(Some(e.to_string()), Vec::new());
            }
            Some(Ok(false)) => {}
            None => self.retry(),
        }
    }
}

mod disk;
mod kept;
#[cfg(test)]
mod owed;
mod placed;
#[cfg(test)]
mod restored;
#[cfg(test)]
mod tests;
mod waits;
