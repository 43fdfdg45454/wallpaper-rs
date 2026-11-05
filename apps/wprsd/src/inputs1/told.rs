use crate::compositor::lock;
use crate::daemon::Daemon;
use crate::error::{Result, invalid};
use crate::inputs::{Hub, reported};
use crate::items::reply;
use std::sync::Arc;
use wprs_core::model::{Input, InputsReply, MODEL};

pub(super) fn inputs(d: &Daemon, hub: &Hub) -> Result<String> {
    let (serial, tracking) = (d.changes.serial(), *hub.tracking.borrow());
    let control = lock(&hub.control);
    let grants = reported(&control);
    let mut shown = control.shown.clone();
    shown.values_mut().for_each(|s| s.granted = control.granted(&s.scene, &s.key));
    let (parallax, declined, capturing) =
        (control.parallax.clone(), control.declined.clone(), control.capturing);
    let clicks = control.views.iter().filter(|(_, v)| v.clicks).map(|(name, _)| name.clone()).collect();
    let clicking = control.wants(Input::Clicks);
    drop(control);
    let unreached = d.unreached();
    let (asking, quiet) = crate::confirm::questions();
    reply(InputsReply {
        model: MODEL,
        serial,
        capturing,
        tracking,
        grants,
        shown,
        parallax,
        declined,
        asking,
        quiet,
        clicks,
        clicking,
        unreached,
        media: crate::media::status(),
    })
}

pub(super) async fn simulate(output: String, track: String, cover: Vec<u8>) -> Result<()> {
    let output = output.trim().to_owned();
    if output.is_empty() || output.len() > 128 || output.chars().any(char::is_control) {
        return Err(invalid("name the screen whose wallpaper gets the made-up track"));
    }
    if track.trim().is_empty() {
        crate::media::simulate(&output, None);
        return Ok(());
    }
    if track.len() > 64 << 10 {
        return Err(invalid("the made-up track is larger than 64 KiB"));
    }
    let made = tokio::task::spawn_blocking(move || crate::drafts::media::simulated(&track, &cover)).await;
    let frame = made.map_err(|e| invalid(e.to_string()))?.map_err(|e| invalid(format!("{e:#}")))?;
    crate::media::simulate(&output, Some(Arc::new(frame)));
    Ok(())
}
