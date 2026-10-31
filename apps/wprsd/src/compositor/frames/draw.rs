use super::super::output::{Output, Sink, smoothstep};
use super::Shared;
use anyhow::Context;
use std::time::Instant;
use wprs_pipewire::{Grant, VideoFrame};
use wprs_render::{Exported, RenderError};

pub(super) fn draw(
    name: &str,
    output: &mut Output,
    shared: Shared<'_>,
    transition: f32,
    now: Instant,
) -> anyhow::Result<bool> {
    let (renderer, exporter, players, power, limits) = shared;
    let moved = limits.map(|steps| (output.offset, steps));
    let refresh = output.refresh;
    if let Some(scene) = output.shown.scene.as_mut()
        && scene.render(renderer, (output.size, refresh), power, players, output.active, now)?
    {
        super::super::feed::declare(name, Some(scene));
    }
    let unchanged = output.unchanged(moved, exporter.starved(name));
    let progress = output.progress(now, output.shown.transition.map_or(transition, |d| d.as_secs_f32()));
    let opacity = if output.fade.is_some() { smoothstep(progress) } else { 1.0 };
    let shown = output.shown.layer(players, opacity, moved, output.size);
    let from = output.fade.as_ref().map(|(from, _)| from.layer(players, 1.0, moved, output.size));
    let layers = [from.unwrap_or(shown), shown];
    let layers = &layers[usize::from(from.is_none())..];
    let damage = output.damage();
    let written = match &mut output.sink {
        _ if unchanged => Ok(Ok(())),
        Sink::Stream(target) => match exporter.lease(name) {
            Grant::Buffer(mut lease) => {
                *target = None;
                (lease.damage, _) = (damage, output.ages.push(damage));
                let image = lease.image.downcast_ref::<Exported>().context("foreign stream buffer")?;
                renderer.render_exported(image, layers, &output.ages).map(|()| exporter.present(lease))
            }
            Grant::Wait => {
                output.last_render = Some(now);
                return Ok(false);
            }
            Grant::Copy => {
                let target = match target {
                    Some(target) => target,
                    empty => empty.insert(renderer.create_target(output.size.0, output.size.1)?),
                };
                output.ages.push(damage);
                renderer.render_damaged(target, layers, &output.ages, |w, h, s, damage, data| {
                    exporter.write_damage(name, VideoFrame { width: w, height: h, stride: s, data }, damage)
                })
            }
        },
        Sink::Surface(screen) => match screen.present(renderer, layers, name) {
            Ok(false) => return Ok(false),
            presented => presented.map(|_| Ok(())),
        },
    };
    if let Err(RenderError::DeviceLost(reason)) = &written {
        super::super::stops::lost(&format!("the GPU stopped responding ({reason})"));
    }
    written??;
    output.missing_frame = output.shown.content.video().is_some() && shown.image.is_none();
    output.shown.scene.iter_mut().for_each(|s| s.damage = Some(Default::default()));
    (output.last_render, output.failed) = (Some(now), 0);
    output.fade = output.fade.take().filter(|_| progress < 1.0);
    output.dirty = output.fading();
    output.stale = output.missing_frame || output.fade.is_some();
    match (output.animated(), &mut output.sink) {
        (true, Sink::Surface(screen)) => screen.due = None,
        (true, Sink::Stream(_)) => {}
        (false, Sink::Stream(target)) => *target = None,
        (false, Sink::Surface(screen)) => {
            screen.backdrop(renderer, &[output.shown.layer(players, 1.0, moved, output.size)], now)?
        }
    }
    Ok(!unchanged)
}
