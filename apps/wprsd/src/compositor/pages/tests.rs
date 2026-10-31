use super::view::ViewSlot;
use super::*;
use crate::pages::{PLAIN, Privacy, WebPack, shape};
use crate::sceneprops::Props;
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, ftruncate, memfd_create};
use std::sync::Arc;

fn spec(language: bool) -> PageSpec {
    let properties =
        serde_json::from_value(json!({"mood": {"type": "combo", "default": "calm", "label": "Mood",
        "options": [{"value": "calm", "label": {"en": "Calm", "es": "Calma"}}]}}))
        .unwrap();
    let languages = vec!["es_AR.UTF-8".to_owned(), "en".to_owned()];
    let props = Props {
        path: "/p.wpak".into(),
        id: "org.example.page".into(),
        version: "1.0.0".into(),
        title: "Page".into(),
        kind: "web",
        languages: languages.clone(),
        properties,
        presets: Vec::new(),
    };
    let (shape, plain) = (shape(&props, &languages), shape(&props, &[PLAIN.into(), "en".into()]));
    let pack = WebPack {
        path: "/p.wpak".into(),
        props: Arc::new(props),
        entry: "index.html".into(),
        preload: None,
        background: [0; 3],
        network: Vec::new(),
        uses: [false; 3],
        storage_kib: 0,
        key: "k".into(),
        shape,
        plain,
    };
    let privacy = Privacy { language, ..Privacy::default() };
    let (files, folders, values) = (Vec::new(), Vec::new(), Default::default());
    PageSpec { pack: Arc::new(pack), values, privacy, files, folders, storage: None, cleared: 0 }
}

#[test]
fn a_page_that_may_not_see_the_language_gets_plain_english() {
    let (shown, hidden) = (spec(true), spec(false));
    let (locale, shape) = language(&shown);
    assert_eq!((locale.as_str(), &shape["labels"]["mood"]["calm"]), ("es_AR.UTF-8", &json!("Calma")));
    let (locale, shape) = language(&hidden);
    assert_eq!((locale.as_str(), &shape["labels"]["mood"]["calm"]), ("en-US", &json!("Calm")));
}

#[test]
fn frames_are_read_only_from_sealed_buffers_of_their_size() {
    let made = |len: u64, seal: bool| {
        let fd = memfd_create("frames", MemfdFlags::ALLOW_SEALING).unwrap();
        ftruncate(&fd, len).unwrap();
        if seal {
            fcntl_add_seals(&fd, SealFlags::SHRINK | SealFlags::GROW).unwrap();
        }
        fd
    };
    let mut view = ViewSlot::new(1, (4, 4));
    assert!(view.map(&made(128, true), (4, 4), 16).is_ok());
    assert!(view.map(&made(128, false), (4, 4), 16).is_err());
    assert!(view.map(&made(64, true), (4, 4), 16).is_err());
}
