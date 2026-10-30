use super::{Hub, hub};
use crate::compositor::lock;
use wprs_core::model::Input;

pub(crate) fn relief(output: &str, pointer: bool) {
    if let Some(hub) = hub() {
        hub.relief(output, pointer)
    }
}

pub(crate) fn pointer_over(outputs: &[&str]) -> Option<[f32; 2]> {
    hub()?.pointer_over(outputs)
}

impl Hub {
    fn relief(&self, output: &str, pointer: bool) {
        let mut control = lock(&self.control);
        let changed = match pointer {
            true => control.parallax.insert(output.into()),
            false => control.parallax.remove(output),
        };
        drop(control);
        if changed {
            self.poke.notify_one();
        }
    }

    fn pointer_over(&self, outputs: &[&str]) -> Option<[f32; 2]> {
        let control = lock(&self.control);
        let pointers = lock(&self.pointers);
        let rect = |output: &str| control.views.get(output).and_then(|v| v.rect);
        let area = outputs.iter().filter_map(|o| rect(o)).reduce(|a, r| {
            let (x, y) = (a[0].min(r[0]), a[1].min(r[1]));
            [x, y, (a[0] + a[2]).max(r[0] + r[2]) - x, (a[1] + a[3]).max(r[1] + r[3]) - y]
        });
        let seen = outputs.iter().filter(|o| control.allows(o, Input::Pointer));
        seen.copied().find_map(|output| {
            let at = pointers.get(output)?.pos?.map(f64::from);
            let placed = match (rect(output), area) {
                (Some(r), Some(a)) => [0, 1].map(|i| (r[i] + at[i] * r[i + 2] - a[i]) / a[i + 2].max(1.0)),
                _ => at,
            };
            Some(placed.map(|v| (v * 2.0 - 1.0).clamp(-1.0, 1.0) as f32))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Hub, View};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    #[test]
    fn a_pointer_parallax_follows_the_pointer_without_a_grant() {
        let hub = Hub::new(Default::default(), Arc::new(|| {}), Arc::new(|| {}));
        let view =
            |x| View { active: true, covered: false, rect: Some([x, 0.0, 100.0, 50.0]), clicks: false };
        let views = |covered| {
            let right = View { covered, ..view(100.0) };
            Some(BTreeMap::from([("DP-1".to_owned(), view(0.0)), ("DP-2".to_owned(), right)]))
        };
        hub.update(views(false));
        assert!(!*hub.tracking.borrow(), "no image asks for the pointer");
        hub.relief("DP-1", true);
        hub.relief("DP-2", true);
        hub.update(views(false));
        assert!(*hub.tracking.borrow());
        hub.set_pointer(150.0, 25.0);
        assert_eq!(hub.pointer_over(&["DP-1", "DP-2"]), Some([0.5, 0.0]), "spanned screens share one area");
        assert_eq!(hub.pointer_over(&["DP-2"]), Some([0.0, 0.0]));
        hub.update(views(true));
        assert_eq!(hub.pointer_over(&["DP-1", "DP-2"]), None, "a covered screen forgets the pointer");
        hub.relief("DP-1", false);
        hub.relief("DP-2", false);
        hub.update(None);
        assert!(!*hub.tracking.borrow());
    }
}
