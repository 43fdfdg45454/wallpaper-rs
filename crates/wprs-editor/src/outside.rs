use crate::action::Action;
use anyhow::Result;
use serde_json::Value;
use std::cell::RefCell;

thread_local! {
    static SCENE: RefCell<Option<(String, Value)>> = const { RefCell::new(None) };
}

pub(crate) fn whole(action: &Action) -> bool {
    let outside = |pointer: &str| !pointer.starts_with(crate::layers::LAYERS);
    match action {
        Action::Set { pointer, .. }
        | Action::Add { pointer, .. }
        | Action::Remove { pointer }
        | Action::Translate { pointer, .. } => outside(pointer),
        Action::Move { from, to } => outside(from) && outside(to),
        _ => matches!(
            action,
            Action::AddProperty { .. }
                | Action::MoveProperty { .. }
                | Action::RenameProperty { .. }
                | Action::RemoveProperty { .. }
                | Action::Property(_)
                | Action::Condition(_)
                | Action::EffectRule(_)
                | Action::AddPreset { .. }
                | Action::Import { .. }
                | Action::ImportEffect { .. }
                | Action::ImportScript(_)
                | Action::ImportAssets(_)
                | Action::Asset(_)
                | Action::Thumbnail { .. }
                | Action::Upgrade(_)
                | Action::Manifest(_)
                | Action::Canvas(_)
                | Action::Timeline(_)
                | Action::Ambient(_)
                | Action::Wind(_)
                | Action::Particles(_)
        ),
    }
}

pub(crate) fn within<T>(doc: &Value, scope: Option<&str>, run: impl FnOnce() -> T) -> T {
    let Some(name) = scope else { return run() };
    let layers = doc["scene"]["layers"].clone();
    SCENE.with(|s| *s.borrow_mut() = Some((name.to_owned(), layers)));
    let out = run();
    SCENE.with(|s| *s.borrow_mut() = None);
    out
}

fn layers() -> Option<(String, Value)> {
    SCENE.with(|s| s.borrow().clone())
}

pub(crate) fn layer(doc: &Value, id: &str) -> Result<Value> {
    let found = crate::actions::layer(doc, id).map(|(_, l)| l.clone());
    found.or_else(|e| {
        let (_, scene) = layers().ok_or(e)?;
        let wrapped = serde_json::json!({ "scene": { "layers": scene } });
        Ok(crate::actions::layer(&wrapped, id)?.1.clone())
    })
}

pub(crate) fn every(doc: &Value) -> Vec<Value> {
    let mut all: Vec<Value> = crate::layers::walk(doc).into_iter().map(|(_, l)| l.clone()).collect();
    if let Some((_, scene)) = layers() {
        let wrapped = serde_json::json!({ "scene": { "layers": scene } });
        all.extend(crate::layers::walk(&wrapped).into_iter().map(|(_, l)| l.clone()));
    }
    all
}

pub(crate) fn real(seen: &Value) -> Option<Value> {
    let (name, scene) = layers()?;
    let mut doc = seen.clone();
    let template = doc["scene"]["layers"][0].take();
    *doc.pointer_mut(&crate::scope::root(&name))? = template;
    doc["scene"]["layers"] = scene;
    Some(doc)
}

pub(crate) fn name() -> Option<String> {
    layers().map(|(n, _)| n)
}
