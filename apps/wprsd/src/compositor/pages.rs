use super::state::State;
use crate::pages::PageSpec;
use serde_json::{Map, Value as Json, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wprs_web::{Folder, Settings, Start, ToWeb, token};

mod feed;
mod frames;
mod slot;
#[cfg(test)]
mod tests;
mod view;

pub(super) use slot::PageSlot;
use view::ViewSlot;

const SCAN: Duration = Duration::from_secs(5);
const GRACE: Duration = Duration::from_secs(2);

type Views = Vec<(String, (u32, u32), (bool, bool), u32, PageSpec)>;
type Wanted = BTreeMap<PathBuf, (PageSpec, Views)>;

pub(super) fn values(spec: &PageSpec, salt: &[u8; 32]) -> (Map<String, Json>, Vec<(String, PathBuf)>) {
    let files: Vec<(String, PathBuf)> =
        spec.files.iter().map(|(prop, path)| (token(prop, salt, path), path.clone())).collect();
    let mut values: Map<String, Json> = spec.values.clone().into_iter().collect();
    for (id, value) in values.iter_mut() {
        let file = spec.files.iter().position(|(prop, _)| prop == id).map(|i| json!(files[i].0));
        let folder =
            spec.folders.iter().any(|(prop, ..)| prop == id).then(|| json!(format!("wpak-user://{id}/")));
        if let Some(token) = file.or(folder) {
            *value = token;
        } else if value.as_str().is_some_and(|v| v.starts_with('/')) {
            *value = Json::Null;
        }
    }
    (values, files)
}

pub(super) fn folders(spec: &PageSpec, salt: &[u8; 32]) -> Vec<Folder> {
    let listed = |(prop, dir, all): &(String, PathBuf, bool)| {
        let property = spec.pack.props.properties.get(prop)?;
        let files =
            crate::picks::files(dir, property).into_iter().map(|p| (token(prop, salt, &p), p)).collect();
        Some(Folder { prop: prop.clone(), all: *all, files })
    };
    spec.folders.iter().filter_map(listed).collect()
}

pub(super) fn language(spec: &PageSpec) -> (String, &Json) {
    match spec.privacy.language {
        true => (spec.pack.props.languages.first().cloned().unwrap_or_default(), &spec.pack.shape),
        false => (crate::pages::PLAIN.into(), &spec.pack.plain),
    }
}

impl State {
    fn wanted(&self) -> Wanted {
        let mut wanted = Wanted::new();
        for (name, o) in &self.outputs {
            let visible = (o.active, o.watched(self.exporter.is_streaming(name)));
            for spec in o.layers().filter_map(|l| l.content.page()) {
                let entry =
                    wanted.entry(spec.pack.path.clone()).or_insert_with(|| (spec.clone(), Vec::new()));
                entry.1.push((name.clone(), o.size, visible, o.fps(self.power), spec.clone()));
            }
        }
        wanted
    }

    fn start(&self, slot: &PageSlot) -> Start {
        let (spec, privacy) = (&slot.spec, slot.spec.privacy);
        let (properties, files) = values(spec, &slot.salt);
        let device = self.page_device(slot);
        let shared = slot.shared && device.is_some();
        Start {
            package: spec.pack.path.clone(),
            id: spec.pack.props.id.clone(),
            entry: spec.pack.entry.clone(),
            preload: spec.pack.preload.clone(),
            background: spec.pack.background,
            network: if privacy.network { spec.pack.network.clone() } else { Vec::new() },
            storage: spec.storage.clone().filter(|dir| crate::pages::within(dir, spec.pack.storage_kib)),
            languages: wprs_web::languages(&wprs_core::locale::languages(), !privacy.language),
            mask_gpu: !privacy.gpu,
            device,
            modifiers: if shared { self.renderer.page_modifiers() } else { Vec::new() },
            properties,
            shape: language(spec).1.clone(),
            settings: self.page_settings(slot),
            allowed: Default::default(),
            folders: folders(spec, &slot.salt),
            files,
        }
    }

    fn page_device(&self, slot: &PageSlot) -> Option<String> {
        let hardware = !slot.hidden && !self.renderer.software();
        let node = hardware.then(|| self.renderer.render_node()).flatten();
        crate::pages::helper().and_then(|program| wprs_web::device(&program, node))
    }

    fn page_settings(&self, slot: &PageSlot) -> Settings {
        let fps = slot.views.values().map(|v| v.fps).max().unwrap_or(30).max(1);
        let silent = self.settings.muted || self.locked;
        let volume = if silent { 0.0 } else { self.settings.volume };
        Settings { fps, quality: self.power.tier as u8, locale: language(&slot.spec).0, volume }
    }

    pub(super) fn sync_pages(&mut self) {
        let (now, wanted) = (Instant::now(), self.wanted());
        self.pages.retain(|path, _| wanted.contains_key(path));
        for (path, (spec, views)) in wanted {
            let renew = self.pages.get(&path).is_none_or(|s| {
                let old = (s.spec.privacy, &s.spec.pack.key, s.spec.cleared, &s.spec.storage);
                old != (spec.privacy, &spec.pack.key, spec.cleared, &spec.storage)
            });
            if renew {
                self.pages.insert(path.clone(), PageSlot::new(spec.clone(), now));
            }
            let Some(mut slot) = self.pages.remove(&path) else { continue };
            slot.spec = spec;
            self.sync_views(&mut slot, &views, now);
            if slot.link.is_none() && slot.due.is_some_and(|d| d <= now) {
                slot.due = None;
                let start = self.start(&slot);
                if start.device.is_none() && !slot.hidden {
                    tracing::info!("the graphics card cannot draw web pages, drawing them in memory");
                    slot.hidden = true;
                }
                slot.full = slot.spec.storage.is_some() && start.storage.is_none();
                slot.spawn(start, &self.tx, now);
                if slot.full {
                    tracing::warn!("the web page keeps more than it declared and keeps nothing this time");
                    slot.error = Some(crate::words::web_full());
                }
            }
            if slot.link.is_some() {
                self.sync_link(&mut slot, now);
            }
            self.pages.insert(path, slot);
        }
    }

    fn sync_views(&self, slot: &mut PageSlot, views: &Views, now: Instant) {
        let gone: Vec<String> =
            slot.views.keys().filter(|n| !views.iter().any(|v| &v.0 == *n)).cloned().collect();
        let mut out: Vec<ToWeb> = gone
            .iter()
            .filter_map(|n| slot.views.remove(n))
            .filter(|v| v.opened)
            .map(|v| ToWeb::Close(v.id))
            .collect();
        for (name, size, (active, watched), fps, spec) in views {
            if !slot.views.contains_key(name) {
                slot.views.insert(name.clone(), ViewSlot::new(slot.next, *size));
                slot.next += 1;
            }
            let view = slot.views.get_mut(name).expect("inserted above");
            let unseen = *view.unseen.get_or_insert(now);
            if *watched {
                view.unseen = None;
            }
            let visible = &(*active && (*watched || now < unseen + GRACE));
            if view.opened && view.size != *size {
                out.push(ToWeb::Resize { view: view.id, size: *size, scale: 1.0 });
            }
            if view.opened && view.visible != Some(*visible) {
                out.push(ToWeb::Visible { view: view.id, visible: *visible });
                view.visible = Some(*visible);
            }
            if view.opened && view.fps != *fps {
                out.push(ToWeb::Fps { view: view.id, fps: *fps });
            }
            (view.size, view.fps, view.live, view.spec) = (*size, *fps, *visible, Some(spec.clone()));
        }
        out.iter().for_each(|m| slot.send(m));
    }
}
