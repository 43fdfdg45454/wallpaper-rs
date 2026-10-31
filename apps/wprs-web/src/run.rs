use crate::ffi::{Ptr, get};
use crate::script::tell;
use crate::state::{VIEWS, View, all, live, view};
use serde_json::json;
use std::ffi::c_int;
use std::time::{Duration, Instant};
use wprs_web::{FromWeb, ToWeb};

const ASLEEP: Duration = Duration::from_secs(300);

pub fn handle(message: ToWeb) {
    match message {
        ToWeb::Start(_) => {}
        ToWeb::Open { view, size, scale, properties } => {
            live(|l| l.views.insert(view, properties.clone()));
            match crate::state::FILTER.with(std::cell::Cell::get).is_null() {
                true => crate::state::WAITING
                    .with(|w| w.borrow_mut().push(ToWeb::Open { view, size, scale, properties })),
                false => crate::view::open(view, size, scale),
            }
        }
        ToWeb::Close(id) => close(id),
        ToWeb::Resize { view: id, size, scale } => {
            if let Some(view) = view(id) {
                let a = get();
                unsafe {
                    let top = (a.wpe_view_get_toplevel)(view.wpe);
                    (a.wpe_toplevel_resize)(top, size.0 as c_int, size.1 as c_int);
                    (a.wpe_toplevel_scale_changed)(top, scale.max(0.25));
                }
            }
        }
        ToWeb::Visible { view: id, visible } => view(id).into_iter().for_each(|v| show(&v, visible)),
        ToWeb::Fps { view: id, fps } => view(id).into_iter().for_each(|v| v.fps.set(fps.max(1))),
        ToWeb::Settings(settings) => {
            let muted = c_int::from(settings.volume <= 0.0);
            all().iter().for_each(|v| unsafe { (get().webkit_web_view_set_is_muted)(v.web, muted) });
            tell(&json!({"settings": settings}));
            live(|l| l.settings = settings);
        }
        ToWeb::Properties { view, values } => {
            live(|l| l.views.entry(view).or_default().extend(values.clone()));
            crate::script::tell_view(view, &json!({"properties": values}));
        }
        ToWeb::Allowed(allowed) => live(|l| l.allowed = allowed),
        ToWeb::Audio { left, right } => {
            if live(|l| l.audio && l.allowed.audio) {
                tell(&json!({"audio": [left, right]}));
            }
        }
        ToWeb::Pointer { view: id, at } => view(id).into_iter().for_each(|v| pointer(&v, at)),
        ToWeb::Press { view: id, at, button } => view(id).into_iter().for_each(|v| press(&v, at, button)),
        ToWeb::Folder(folder) => folders(folder),
        ToWeb::Files(files) => live(|l| l.files.extend(files)),
        ToWeb::Action(id) => tell(&json!({"action": id})),
        ToWeb::Released { view: id, buffer } => {
            view(id).into_iter().for_each(|v| crate::frames::release(&v, buffer))
        }
        ToWeb::Freed { view: id, slot } => view(id).into_iter().for_each(|v| crate::frames::freed(&v, slot)),
        ToWeb::Copy => live(|l| l.copy = true),
        ToWeb::Quit => std::process::exit(0),
    }
}

fn folders(folder: wprs_web::Folder) {
    let old = live(|l| {
        l.files.extend(folder.files.iter().map(|(t, p)| (t.clone(), p.clone())));
        l.folders.insert(folder.prop.clone(), folder.clone())
    });
    let old = old.map(|f| f.files).unwrap_or_default();
    let before: Vec<String> = old.iter().map(|(t, _)| t.clone()).collect();
    let now: Vec<String> = folder.files.iter().map(|(t, _)| t.clone()).collect();
    live(|l| before.iter().filter(|t| !now.contains(t)).for_each(|t| drop(l.files.remove(t))));
    let paths: std::collections::HashMap<&str, &std::path::Path> =
        old.iter().chain(&folder.files).map(|(t, p)| (t.as_str(), p.as_path())).collect();
    let path = |t: &str| paths.get(t).map(|p| p.to_path_buf());
    if let Some(notice) = wprs_format::page::files(&folder.prop, &before, &now, path).filter(|_| folder.all) {
        tell(&json!({ "files": notice }));
    }
}

fn close(id: u32) {
    if let Some(view) = VIEWS.with(|v| v.borrow_mut().remove(&id)) {
        let a = get();
        let held: Vec<Ptr> = view.held.borrow_mut().drain().map(|(_, b)| b).collect();
        unsafe {
            held.iter().for_each(|b| (a.wpe_view_buffer_released)(view.wpe, *b));
            (a.webkit_web_view_terminate_web_process)(view.web);
            (a.g_object_unref)(view.web);
            (a.g_object_unref)(view.manager);
        }
    }
}

fn show(view: &std::rc::Rc<View>, visible: bool) {
    if view.visible.replace(visible) == visible {
        return;
    }
    unsafe { (get().wpe_view_set_visible)(view.wpe, c_int::from(visible)) };
    if visible {
        view.hidden_at.set(None);
        if view.stopped.replace(false) {
            view.loaded.set(false);
            crate::view::load(view.id);
        }
        return;
    }
    let at = Instant::now();
    view.hidden_at.set(Some(at));
    let view = view.clone();
    crate::glib::later(ASLEEP.as_millis() as u32, move || {
        if view.hidden_at.get() == Some(at) && !view.visible.get() {
            view.stopped.set(true);
            unsafe { (get().webkit_web_view_terminate_web_process)(view.web) };
            crate::state::send(&FromWeb::Said("the hidden page was stopped to save memory".into()));
        }
    });
}

fn time() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u32)
}

fn pointer(view: &View, at: Option<(f64, f64)>) {
    if !live(|l| l.allowed.pointer || l.allowed.clicks) {
        return;
    }
    let a = get();
    let (kind, (x, y)) = match at {
        Some(at) => (3, at),
        None => (5, (0.0, 0.0)),
    };
    deliver(view, unsafe { (a.wpe_event_pointer_move_new)(kind, view.wpe, 0, time(), 0, x, y, 0.0, 0.0) });
}

fn deliver(view: &View, event: Ptr) {
    if !event.is_null() {
        let a = get();
        unsafe {
            (a.wpe_view_event)(view.wpe, event);
            (a.wpe_event_unref)(event);
        }
    }
}

fn press(view: &View, (x, y): (f64, f64), button: u32) {
    if !live(|l| l.allowed.clicks) {
        return;
    }
    let a = get();
    pointer(view, Some((x, y)));
    for (kind, modifiers, count) in [(1, 1 << (7 + button.clamp(1, 5)), 1), (2, 0, 0)] {
        let event = unsafe {
            (a.wpe_event_pointer_button_new)(kind, view.wpe, 0, time(), modifiers, button, x, y, count)
        };
        deliver(view, event);
    }
}
