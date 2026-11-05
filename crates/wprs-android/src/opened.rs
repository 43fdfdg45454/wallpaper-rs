use crate::pack::Pack;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use wprs_scene::Control;

const CLICK: Duration = Duration::from_secs(2);
const APART: Duration = Duration::from_secs(1);
const WAITING: usize = 4;

static PAGES: Mutex<BTreeMap<i64, (Gate, Vec<Value>)>> = Mutex::new(BTreeMap::new());

#[derive(Debug, Default)]
pub(crate) struct Gate {
    clicked: Option<Instant>,
    opened: Option<Instant>,
}

impl Gate {
    pub(crate) fn clicked(&mut self, now: Instant) {
        self.clicked = Some(now);
    }

    pub(crate) fn lets(&mut self, now: Instant) -> bool {
        let recent = self.clicked.is_some_and(|at| now.duration_since(at) < CLICK);
        let rested = self.opened.is_none_or(|at| now.duration_since(at) >= APART);
        if recent && rested {
            (self.clicked, self.opened) = (None, Some(now));
        }
        recent && rested
    }
}

pub(crate) fn openable(value: &Value) -> bool {
    let Some((kind, text)) = value.as_object().filter(|m| m.len() == 1).and_then(|m| m.iter().next()) else {
        return false;
    };
    let text = text.as_str().unwrap_or_default();
    match kind.as_str() {
        "uri" => text.starts_with("https://") && text.len() <= 2048 && !text.chars().any(char::is_control),
        "app" => {
            (1..=255).contains(&text.len())
                && text.bytes().all(|b| b.is_ascii_alphanumeric() || b"._".contains(&b))
        }
        _ => false,
    }
}

pub(crate) fn chosen(pack: &Pack, prop: &str) -> Option<Value> {
    let shortcut = pack.properties.get(prop).is_some_and(|p| matches!(p.control, Control::Shortcut {}));
    shortcut.then(|| crate::props::values(pack).remove(prop)).flatten().filter(openable)
}

fn pages() -> MutexGuard<'static, BTreeMap<i64, (Gate, Vec<Value>)>> {
    PAGES.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(crate) fn pressed(handle: i64) {
    pages().entry(handle).or_default().0.clicked(Instant::now());
}

pub(crate) fn page(handle: i64, pack: &Pack, prop: &str) {
    let mut pages = pages();
    let (gate, waiting) = pages.entry(handle).or_default();
    let allowed = crate::props::granted(&pack.id)[2] && waiting.len() < WAITING;
    if let Some(target) = chosen(pack, prop).filter(|_| allowed && gate.lets(Instant::now())) {
        waiting.push(target);
    }
}

pub(crate) fn taken(handle: i64) -> Vec<Value> {
    pages().get_mut(&handle).map(|(_, waiting)| std::mem::take(waiting)).unwrap_or_default()
}

pub(crate) fn forget(handle: i64) {
    pages().remove(&handle);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_shortcut_opens_only_soon_after_a_tap_and_not_twice_in_a_second() {
        let (mut gate, start) = (Gate::default(), Instant::now());
        assert!(!gate.lets(start), "nothing was tapped");
        gate.clicked(start);
        assert!(!gate.lets(start + CLICK), "the tap is too old");
        gate.clicked(start + CLICK);
        assert!(gate.lets(start + CLICK + Duration::from_millis(100)));
        gate.clicked(start + CLICK + Duration::from_millis(200));
        assert!(!gate.lets(start + CLICK + Duration::from_millis(300)), "a second opening within a second");
        assert!(gate.lets(start + CLICK + APART + Duration::from_millis(150)));
    }

    #[test]
    fn the_phone_opens_web_addresses_and_apps_only() {
        for good in [json!({ "uri": "https://example.org/a?b=c" }), json!({ "app": "org.example.notes" })] {
            assert!(openable(&good), "{good}");
        }
        let long = format!("https://example.org/{}", "a".repeat(2048));
        for bad in [
            json!({ "uri": "http://example.org" }),
            json!({ "uri": "javascript:alert(1)" }),
            json!({ "uri": long }),
            json!({ "uri": "https://example.org/\n" }),
            json!({ "app": "" }),
            json!({ "app": "org.example/notes" }),
            json!({ "app": "notes-app" }),
            json!({ "file": "/sdcard/a.txt" }),
            json!({ "folder": "/sdcard" }),
            json!({ "uri": "https://a.org", "app": "org.example" }),
            json!("https://example.org"),
            json!(null),
            json!({ "uri": 3 }),
        ] {
            assert!(!openable(&bad), "{bad}");
        }
    }
}
