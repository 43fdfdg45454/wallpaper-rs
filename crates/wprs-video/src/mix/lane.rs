use super::{AHEAD, DRIFT, DUCK, Item, RAMP, RATE, Source, Track, same};
use crate::feed::Slot;
use wprs_scene::{Spot, Voice, Walk};

#[derive(Default)]
pub(super) struct Lane {
    pub(super) track: Track,
    pub(super) sources: Vec<Option<Source>>,
    pub(super) voice: Voice,
    pub(super) aim: f64,
    pub(super) at: f64,
    pub(super) walk: Walk,
    pub(super) item: Option<Item>,
    pub(super) feeds: Vec<(f64, Slot)>,
    pub(super) gain: f32,
    pub(super) level: f32,
    pub(super) pan: f32,
    pub(super) duck: f32,
    pub(super) shift: Option<f64>,
}

impl Lane {
    fn ready(&self) -> bool {
        self.sources.iter().all(Option::is_some) && !self.sources.is_empty()
    }

    pub(super) fn sounding(&self) -> bool {
        !self.ready() || !matches!(self.item, Some((None, _, end)) if end.is_infinite())
    }

    pub(super) fn prepare(&mut self, now: f64) {
        let wanted = self.aim + now;
        let on = self.voice.play || self.gain > 0.0;
        let ended = matches!(self.item, Some((None, _, end)) if end.is_infinite());
        let drift = (wanted - self.at).abs() > DRIFT || (ended && wanted < self.at);
        if !on || (drift && (ended || self.gain <= 0.0)) {
            (self.at, self.duck, self.shift) = (wanted, if on { 0.0 } else { 1.0 }, None);
        } else if drift {
            self.shift = Some(wanted - self.at);
        }
        if self.duck <= 0.0
            && let Some(shift) = self.shift.take()
        {
            self.at += shift;
        }
        self.feeds.iter_mut().for_each(|(_, slot)| slot.settle());
        if !on || !self.ready() {
            return self.feeds.clear();
        }
        self.locate();
        let current = self.item.and_then(|(clip, start, _)| Some((clip?, start, self.at - start)));
        let next = self.next().filter(|(_, start)| start - self.at < AHEAD).map(|(c, start)| (c, start, 0.0));
        let keep = [current, next];
        self.feeds.retain(|(key, _)| keep.iter().flatten().any(|(_, start, _)| same(*key, *start)));
        for (clip, start, offset) in keep.into_iter().flatten() {
            let Some(Source::Stream(path)) = &self.sources[clip] else { continue };
            if !self.feeds.iter().any(|(key, _)| same(*key, start)) {
                self.feeds.push((start, Slot::open(path, offset.max(0.0) as u64)));
            }
        }
    }

    fn next(&self) -> Option<(usize, f64)> {
        let (rate, mut walk, (_, _, mut end)) = (f64::from(RATE), self.walk, self.item?);
        if end.is_infinite() {
            return None;
        }
        for _ in 0..2 {
            match self.track.list.spot(end / rate, &mut walk) {
                Spot::Clip { index, .. } => return Some((index, end)),
                Spot::Gap(left) => end += left * rate,
                Spot::End => return None,
            }
        }
        None
    }

    fn locate(&mut self) {
        let fresh = self.item.is_some_and(|(_, start, end)| (start..end).contains(&self.at));
        if fresh || !self.ready() {
            return;
        }
        let rate = f64::from(RATE);
        self.item = Some(match self.track.list.spot(self.at / rate, &mut self.walk) {
            Spot::Clip { index, offset, left } => {
                (Some(index), self.at - offset * rate, self.at + left * rate)
            }
            Spot::Gap(left) => (None, self.at, self.at + (left * rate).max(1.0)),
            Spot::End => (None, self.at, f64::INFINITY),
        });
    }

    fn sample(&mut self) -> Option<[f32; 2]> {
        self.locate();
        let (clip, start, _) = self.item?;
        let offset = (self.at - start).max(0.0) as usize;
        match self.sources.get(clip?)?.as_ref()? {
            Source::Memory(c) => {
                let at = offset * c.channels;
                Some([c.samples.get(at)?, c.samples.get(at + c.channels - 1)?].map(|s| f32::from(*s)))
            }
            Source::Stream(_) => {
                let (_, slot) = self.feeds.iter_mut().find(|(key, _)| same(*key, start))?;
                slot.frame(offset as u64).map(|s| s.map(f32::from))
            }
        }
    }

    pub(super) fn add(&mut self, sum: &mut [f32; 2]) {
        let on = self.voice.play;
        if !on && self.gain <= 0.0 {
            return;
        }
        let fade = if on { self.track.fade_in } else { self.track.fade_out };
        let step = if fade > 0.0 { 1.0 / (fade * RATE as f32) } else { 1.0 };
        self.gain = if on { (self.gain + step).min(1.0) } else { (self.gain - step).max(0.0) };
        self.level += (self.voice.volume - self.level).clamp(-RAMP, RAMP);
        self.pan += (self.voice.pan - self.pan).clamp(-RAMP, RAMP);
        self.duck = match self.shift {
            Some(_) => (self.duck - DUCK).max(0.0),
            None => (self.duck + DUCK).min(1.0),
        };
        let heard = self.sample();
        self.at += 1.0;
        let Some([left, right]) = heard else { return };
        let gain = self.gain * self.level * self.duck / 32_768.0;
        sum[0] += left * gain * (1.0 - self.pan).min(1.0);
        sum[1] += right * gain * (1.0 + self.pan).min(1.0);
    }
}
