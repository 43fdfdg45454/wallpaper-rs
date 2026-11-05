use super::{Ask, Status};
use crate::compositor::lock;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wprs_core::rgb::Rgb;
use wprs_rgb::{Client, Strip, colors, pattern};

const WAIT: Duration = Duration::from_secs(2);
const RETRY: Duration = Duration::from_secs(10);
const TEST: Duration = Duration::from_secs(3);
const STEP: Duration = Duration::from_millis(33);
const LISTEN: Duration = Duration::from_secs(1);
const IDLE: Duration = Duration::from_secs(3600);

struct Session {
    status: Arc<Mutex<Status>>,
    rgb: Rgb,
    client: Option<Client>,
    failed: Option<Instant>,
    last: Option<(String, Strip)>,
    test: Option<(Instant, Option<u32>)>,
}

pub(super) fn spawn(asks: Receiver<Ask>, status: Arc<Mutex<Status>>) {
    let session = Session { status, rgb: Rgb::default(), client: None, failed: None, last: None, test: None };
    let started = std::thread::Builder::new().name("wprsd-rgb".into()).spawn(move || run(asks, session));
    if let Err(e) = started {
        tracing::warn!("cannot start the RGB lighting: {e}");
    }
}

fn run(asks: Receiver<Ask>, mut s: Session) {
    loop {
        lock(&s.status).held = s.held();
        match asks.recv_timeout(s.wait()) {
            Ok(Ask::Settings(rgb)) => {
                if rgb != s.rgb {
                    if !rgb.enabled {
                        s.forget(None);
                    }
                    (s.client, s.failed, s.rgb) = (None, None, rgb);
                    s.publish(None);
                    s.retry();
                }
            }
            Ok(Ask::Forget(source)) => s.forget(Some(&source)),
            Ok(Ask::Covered) => {}
            Ok(Ask::Frame(source, strip)) => {
                lock(&s.status).source = Some(source.clone());
                s.last = Some((source, strip.clone()));
                if s.test.is_none() {
                    s.paint(&strip, None);
                }
            }
            Ok(Ask::Connect(reply)) => {
                (s.client, s.failed) = (None, None);
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
                    s.test = Some((Instant::now(), device));
                    lock(&s.status).testing = true;
                }
                reply.send(ready).ok();
            }
            Err(RecvTimeoutError::Timeout) => s.tick(),
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

impl Session {
    fn forget(&mut self, source: Option<&str>) {
        if self.last.as_ref().is_some_and(|(from, _)| source.is_none_or(|s| s == from)) {
            self.last = None;
            lock(&self.status).source = None;
        }
    }

    fn repaint(&mut self) {
        match self.last.clone() {
            Some((source, strip)) if super::allowed(&source) => self.paint(&strip, None),
            Some(_) => self.forget(None),
            None => {}
        }
    }

    fn publish(&mut self, error: Option<String>) {
        let mut status = lock(&self.status);
        let was = status.receiving();
        status.connected = self.client.is_some();
        status.error = error;
        if self.client.is_none() {
            status.devices.clear();
        }
        let now = status.receiving();
        drop(status);
        if was != now {
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
        let opened = Client::connect(&self.rgb.server, WAIT).and_then(|mut c| {
            let devices = c.devices()?;
            devices.iter().try_for_each(|d| c.direct(d.index))?;
            Ok((c, devices))
        });
        match opened {
            Ok((client, devices)) => {
                self.client = Some(client);
                lock(&self.status).devices = devices;
                self.publish(None);
                Ok(())
            }
            Err(e) => {
                (self.client, self.failed) = (None, Some(Instant::now()));
                self.publish(Some(e.to_string()));
                Err(e.to_string())
            }
        }
    }

    fn paint(&mut self, strip: &Strip, only: Option<u32>) {
        if self.ready().is_err() {
            return;
        }
        let devices = lock(&self.status).devices.clone();
        let Some(client) = self.client.as_mut() else { return };
        let wanted = devices.iter().filter(|d| only.is_none_or(|i| i == d.index));
        let painted = wanted.into_iter().try_for_each(|d| client.paint(d.index, &colors(d, strip)));
        match painted {
            Ok(()) => lock(&self.status).frames += 1,
            Err(e) => {
                (self.client, self.failed) = (None, Some(Instant::now()));
                self.publish(Some(e.to_string()));
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
            return self.repaint();
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
                self.publish(Some(e.to_string()));
            }
            Some(Ok(false)) => {}
            None => self.retry(),
        }
    }
}

#[cfg(test)]
mod tests;
mod waits;
