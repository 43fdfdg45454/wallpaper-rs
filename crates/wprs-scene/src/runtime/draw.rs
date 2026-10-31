use super::Runtime;
use crate::eval::{Frame, walk};
use crate::particles::plan;
use crate::resolve::Context;
use crate::spare::Spare;
use crate::timeline::Playhead;
use crate::{Affine, Layer, Quality, Wake};
use std::cell::Cell;
use std::collections::BTreeSet;

impl Runtime {
    pub(super) fn trigger(&mut self, matches: impl Fn(&str) -> bool) {
        self.trigger_late(matches, 0.0);
    }

    pub(crate) fn trigger_late(&mut self, matches: impl Fn(&str) -> bool, late: f32) -> Vec<usize> {
        let mut started = Vec::new();
        for (i, (tl, head)) in self.timelines.iter_mut().enumerate() {
            if tl.trigger.as_deref().is_some_and(&matches) {
                *head = Playhead {
                    t: tl.origin(head.rate) + late * head.rate,
                    playing: true,
                    reversed: false,
                    ..*head
                };
                started.push(i);
            }
        }
        let t = self.t - f64::from(late);
        let sounds = self.scene.sounds().map(|s| s.play_on.as_deref().is_some_and(&matches));
        self.onsets.iter_mut().zip(sounds).filter(|(_, on)| *on).for_each(|(onset, _)| *onset = Some(t));
        started
    }

    pub(crate) fn context(&self) -> Context<'_> {
        Context {
            t: self.t as f32,
            dt: self.dt,
            calm: self.stirred.map(|s| (self.t - s) as f32),
            beat: self.since_beat(),
            wall: self.wall,
            step: f64::from(self.scene.clock.step.unwrap_or(60.0)).max(1.0),
            lang: &self.lang,
            audio: &self.audio,
            pointer: &self.pointer,
            script: &self.script,
            props: &self.props,
            timelines: &self.timelines,
            mattes: &self.mattes,
            playing: &self.playing,
            memo: &self.memo,
            wake: Cell::new(Wake::Idle),
            pace: Cell::new(0),
            scene: &self.scene,
            homes: &self.homes,
            places: {
                self.places.borrow_mut().clear();
                &self.places
            },
            motion: &self.motion,
            now: self.t,
            view: [0.0; 4],
            canvas: self.scene.canvas.size.map(|v| v as f32),
            glide: &self.glide,
            tilt: if self.still { [0.0; 2] } else { self.tilt },
            still: self.still,
            sheets: &self.sheets,
            flocks: &self.flocks,
        }
    }

    pub fn frame(&self, view: [f32; 4], quality: Quality) -> Frame<'_> {
        self.frame_in(Spare::default(), view, quality)
    }

    pub fn frame_in(&self, spare: Spare, view: [f32; 4], quality: Quality) -> Frame<'_> {
        let cx = Context { view, ..self.context() };
        let canvas = self.scene.canvas.size.map(|v| v as f32);
        let (mut draws, mut effects) = spare.lists();
        draws.reserve(self.sizes.get().0);
        effects.reserve(self.sizes.get().1);
        let background = self.scene.canvas.background;
        let (time, wake) = (self.t as f32, Wake::Idle);
        let mut frame = Frame { draws, effects, time, view, background, quality, wake };
        match self.arranged() {
            Some(top) => walk(&cx, top, Affine::IDENTITY, quality, &mut frame, canvas),
            None => walk(&cx, &self.scene.layers, Affine::IDENTITY, quality, &mut frame, canvas),
        }
        plan(&self.swarms, (&self.systems, &self.flocks), &cx, &frame, quality, self.t);
        frame.wake = cx.wake.get().min(self.cue_wake());
        self.pace.set(if frame.wake == Wake::Frame { cx.pace.get() } else { u32::MAX });
        self.sizes.set((frame.draws.len(), frame.effects.len()));
        self.wake.set(frame.wake);
        frame
    }
}

pub(super) fn holds(layers: &[Layer], targets: &BTreeSet<String>, found: &mut BTreeSet<String>) -> bool {
    let mut any = false;
    for layer in layers {
        if holds(layer.children(), targets, found) || targets.contains(&layer.id) {
            found.insert(layer.id.clone());
            any = true;
        }
    }
    any
}
