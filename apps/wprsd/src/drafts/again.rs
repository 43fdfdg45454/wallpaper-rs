use serde_json::Value;

fn stripped(document: &str) -> Option<(Value, Value)> {
    let mut doc: Value = serde_json::from_str(document).ok()?;
    let nonce = doc["preview"].as_object_mut().and_then(|p| p.remove("restart")).unwrap_or(Value::Null);
    Some((doc, nonce))
}

pub(super) fn restarted(old: &str, new: &str) -> bool {
    if old.len().abs_diff(new.len()) > 32 || !new.contains("\"restart\"") {
        return false;
    }
    let (Some((old, before)), Some((new, after))) = (stripped(old), stripped(new)) else { return false };
    !after.is_null() && after != before && old == new
}

#[cfg(test)]
mod tests {
    use super::restarted;

    #[test]
    fn only_a_new_restart_nonce_restarts_the_particles_of_a_draft() {
        let doc = |nonce: u64, size: u32| {
            format!(
                r#"{{"scene": {{"canvas": {{"size": [{size}, 10]}}}}, "preview": {{"quality": "q1", "restart": {nonce}}}}}"#
            )
        };
        assert!(restarted(&doc(1, 10), &doc(2, 10)));
        assert!(!restarted(&doc(1, 10), &doc(1, 10)), "the same nonce is the same preview");
        assert!(!restarted(&doc(1, 10), &doc(2, 20)), "a changed scene is built again");
        assert!(!restarted(r#"{"scene": {}}"#, r#"{"scene": {}}"#), "no nonce restarts nothing");
        assert!(!restarted("not json", &doc(2, 10)));
    }
}
