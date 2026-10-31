use crate::ffi::{Ptr, get, text};
use crate::state::{all, engine, live, send};
use serde_json::{Value, json};
use std::ffi::c_int;
use wprs_web::FromWeb;

const POST: &str = "(m) => window.webkit.messageHandlers.wprs.postMessage(JSON.stringify(m))";
const MAX_SAID: u32 = 200;
const MEDIA: &str = include_str!("media.js");
const ALL: c_int = 0;
const TOP: c_int = 1;

fn source(view: u32) -> Vec<(String, c_int)> {
    let Some(engine) = engine() else { return Vec::new() };
    let init = live(|l| {
        let s = &l.settings;
        json!({
            "properties": l.views.get(&view).unwrap_or(&l.properties),
            "kinds": engine.start.shape.get("kinds").cloned().unwrap_or(json!({})),
            "labels": engine.start.shape.get("labels").cloned().unwrap_or(json!({})),
            "settings": {"fps": s.fps, "quality": s.quality, "locale": s.locale, "volume": s.volume},
        })
    });
    let mut scripts = vec![(wprs_format::page::api(&init, POST), TOP)];
    if engine.start.mask_gpu {
        scripts.push((format!("({})(globalThis);", wprs_format::page::MASK.trim_end()), TOP));
    }
    scripts.push((wprs_format::page::VOLUME.into(), ALL));
    scripts.push((MEDIA.into(), ALL));
    scripts.extend(engine.preload.clone().map(|p| (p, TOP)));
    scripts
}

pub fn install(view: u32, manager: Ptr) {
    let a = get();
    unsafe {
        (a.webkit_user_content_manager_remove_all_scripts)(manager);
        for (source, frames) in source(view) {
            let source = text(&source);
            let script =
                (a.webkit_user_script_new)(source.as_ptr(), frames, 0, std::ptr::null(), std::ptr::null());
            (a.webkit_user_content_manager_add_script)(manager, script);
            (a.webkit_user_script_unref)(script);
        }
    }
}

pub fn tell(message: &Value) {
    all().iter().for_each(|v| tell_view(v.id, message));
}

pub fn tell_view(id: u32, message: &Value) {
    let code = text(&format!("window.__wprs && window.__wprs({message})"));
    let a = get();
    for view in crate::state::view(id).into_iter().filter(|v| v.loaded.get() && !v.stopped.get()) {
        unsafe {
            let null = std::ptr::null_mut();
            (a.webkit_web_view_evaluate_javascript)(
                view.web,
                code.as_ptr(),
                -1,
                std::ptr::null(),
                std::ptr::null(),
                null,
                null,
                null,
            )
        };
    }
}

fn reply(view: u32, id: u64, value: Value) {
    let Some(view) = crate::state::view(view) else { return };
    let code = text(&format!("window.__wprs({})", json!({"reply": {"id": id, "value": value}})));
    unsafe {
        let null = std::ptr::null_mut();
        (get().webkit_web_view_evaluate_javascript)(
            view.web,
            code.as_ptr(),
            -1,
            std::ptr::null(),
            std::ptr::null(),
            null,
            null,
            null,
        )
    };
}

fn tokens(prop: &str, every: bool) -> Option<Vec<String>> {
    live(|l| {
        let folder = l.folders.get(prop)?;
        (every == folder.all || !every).then(|| folder.files.iter().map(|(t, _)| t.clone()).collect())
    })
}

fn random(list: &[String]) -> Value {
    let seed =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    match list.len() {
        0 => Value::Null,
        n => Value::String(list[seed as usize % n].clone()),
    }
}

pub unsafe extern "C" fn message(_manager: Ptr, value: Ptr, data: Ptr) {
    let a = get();
    let raw = unsafe { (a.jsc_value_to_string)(value) };
    let text = unsafe { crate::glib::string(raw) };
    unsafe { (a.g_free)(raw.cast()) };
    let Ok(Value::Object(message)) = serde_json::from_str::<Value>(&text) else { return };
    let (view, id) = (data as usize as u32, message.get("id").and_then(Value::as_u64).unwrap_or(0));
    if message.get("audio").is_some() && !live(|l| std::mem::replace(&mut l.audio, true)) {
        send(&FromWeb::Wants { audio: true });
    }
    if let Some(prop) = message.get("files").and_then(Value::as_str) {
        reply(view, id, tokens(prop, true).map_or(Value::Null, |t| json!(t)));
    }
    if let Some(prop) = message.get("random").and_then(Value::as_str) {
        reply(view, id, random(&tokens(prop, false).unwrap_or_default()));
    }
    if let Some(error) = message.get("error").and_then(Value::as_str) {
        let quiet = live(|l| {
            l.said += 1;
            l.said > MAX_SAID
        });
        if !quiet {
            send(&FromWeb::Said(error.chars().take(512).collect()));
        }
    }
}
