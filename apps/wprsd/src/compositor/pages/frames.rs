use super::super::lock;
use super::super::state::State;
use super::slot::{FIRST_FRAME, PageSlot, RETRIES};
use super::view::Shared;
use super::{SCAN, folders, values};
use std::time::Instant;
use wprs_web::{FromWeb, ToWeb};

impl State {
    pub(super) fn sync_link(&self, slot: &mut PageSlot, now: Instant) {
        let (mut out, mut files) = (Vec::new(), Vec::new());
        for view in slot.views.values_mut() {
            let (values, own) = values(view.spec.as_ref().unwrap_or(&slot.spec), &slot.salt);
            files.extend(own);
            let changed = wprs_web::changed(&view.sent, &values);
            if view.opened && !changed.is_empty() {
                out.push(ToWeb::Properties { view: view.id, values: changed });
            }
            view.sent = values;
        }
        if files != slot.files {
            out.insert(0, ToWeb::Files(files.clone()));
            slot.files = files;
        }
        for view in slot.views.values_mut().filter(|v| !v.opened) {
            view.opened = true;
            let properties = view.sent.clone();
            out.push(ToWeb::Open { view: view.id, size: view.size, scale: 1.0, properties });
            out.push(ToWeb::Fps { view: view.id, fps: view.fps.max(1) });
            if !view.live {
                out.push(ToWeb::Visible { view: view.id, visible: false });
            }
            view.visible = Some(view.live);
        }
        let settings = self.page_settings(slot);
        if settings != slot.settings {
            out.push(ToWeb::Settings(settings.clone()));
            slot.settings = settings;
        }
        let shown = slot.views.values().any(|v| v.live);
        if shown && slot.scanned.is_none_or(|at| now.duration_since(at) >= SCAN) {
            slot.scanned = Some(now);
            for folder in folders(&slot.spec, &slot.salt) {
                if slot.folders.get(&folder.prop) != Some(&folder) {
                    out.push(ToWeb::Folder(folder.clone()));
                    slot.folders.insert(folder.prop.clone(), folder);
                }
            }
        }
        out.iter().for_each(|m| slot.send(m));
        match slot.waiting {
            Some(at) if at <= now && !shown => slot.waiting = Some(now + FIRST_FRAME),
            Some(at) if at <= now && !slot.hidden => {
                tracing::info!(
                    "the web page drew nothing on the graphics card, drawing it in memory instead"
                );
                (slot.hidden, slot.due) = (true, Some(now));
                slot.stop();
            }
            Some(at) if at <= now => slot.fail(crate::words::web_stalled(), now),
            _ => {}
        }
    }

    pub(in crate::compositor) fn take_page_frames(&mut self) {
        let now = Instant::now();
        let renderer = self.renderer;
        for slot in self.pages.values_mut() {
            let messages = std::mem::take(&mut *lock(&slot.inbox));
            for (message, fds) in messages {
                take(slot, renderer, message, fds, now);
            }
        }
        for (name, output) in &mut self.outputs {
            let Some(path) = output.shown.content.page().map(|p| p.pack.path.clone()) else { continue };
            let view = self.pages.get_mut(&path).and_then(|s| s.views.get_mut(name));
            if let Some(view) = view.filter(|v| v.fresh) {
                view.fresh = false;
                output.shown.image = view.texture.clone();
                output.dirty = true;
            }
        }
    }
}

fn view_of(slot: &mut PageSlot, id: u32) -> Option<&mut super::ViewSlot> {
    slot.views.values_mut().find(|v| v.id == id)
}

fn take(
    slot: &mut PageSlot,
    renderer: &wprs_render::Renderer,
    message: FromWeb,
    fds: Vec<std::os::fd::OwnedFd>,
    now: Instant,
) {
    match message {
        FromWeb::Ready { version, .. } => tracing::debug!(version, "the web helper started"),
        FromWeb::Unavailable(reason) => {
            slot.failures = RETRIES;
            slot.fail(reason.clone(), now);
            slot.error = Some(crate::words::web_old(&reason));
        }
        FromWeb::Failed(reason) => slot.fail(reason, now),
        FromWeb::Buffer { view, id, size, fourcc, modifier, offset, stride } => {
            let shared = Shared { id, size, fourcc, modifier, offset, stride };
            let imported = match (view_of(slot, view), fds.first()) {
                (Some(v), Some(fd)) => v.import(renderer, &shared, fd).map_err(|e| e.to_string()),
                _ => Err("no buffer".into()),
            };
            if let Err(e) = imported
                && slot.shared
            {
                tracing::info!("the GPU cannot read the web page in place, copying it instead: {e}");
                slot.shared = false;
                slot.send(&ToWeb::Copy);
            }
        }
        FromWeb::Forget { view } => view_of(slot, view).into_iter().for_each(|v| v.forget()),
        FromWeb::Frame { view, buffer } => {
            let shown = view_of(slot, view).map(|v| (v.frame(buffer), v.shown == Some(buffer)));
            let release = match shown {
                Some((old, true)) => old,
                _ => Some(buffer),
            };
            release.into_iter().for_each(|buffer| slot.send(&ToWeb::Released { view, buffer }));
            drawn(slot);
        }
        FromWeb::Pixels { view, slot: index, size, stride, fresh } => {
            if let (Some(v), Some(fd)) = (view_of(slot, view), fds.first().filter(|_| fresh)) {
                let _ = v.map(fd, size, stride);
            }
            let uploaded = view_of(slot, view).map(|v| v.pixels(renderer, index, size));
            if let Some(Err(e)) = uploaded {
                tracing::debug!("cannot upload a web frame: {e}");
            }
            slot.send(&ToWeb::Freed { view, slot: index });
            drawn(slot);
        }
        FromWeb::Wants { audio } => slot.audio = audio,
        FromWeb::Loaded { .. } => tracing::debug!("the web page loaded"),
        FromWeb::Crashed { .. } => tracing::warn!("the web page crashed and is loading again"),
        FromWeb::Said(text) if text == "sandbox: off" => slot.sandbox = "reduced",
        FromWeb::Said(text) => tracing::debug!("web page: {text}"),
    }
}

fn drawn(slot: &mut PageSlot) {
    if slot.waiting.take().is_some() {
        (slot.failures, slot.error) = (0, None);
    }
}
