use super::*;

#[test]
fn timelines_count_frames_wrap_and_seeds_start_fresh() {
    let counted = scene(
        json!({ "timelines": [{ "id": "intro", "fps": 24, "frames": 48, "events": [{ "t": 1.5, "name": "drop" }] }] }),
    );
    assert_eq!((counted.timelines[0].length(), counted.validate(&BTreeMap::new())), (2.0, Vec::new()));
    assert!(
        counted.features().contains("timeline_frames") && !counted.to_json().unwrap().contains("duration")
    );
    let wrapped =
        scene(json!({ "timelines": [{ "id": "intro", "duration": 2, "loop": "repeat", "wrap": true }] }));
    assert!(wrapped.validate(&BTreeMap::new()).is_empty() && wrapped.features().contains("timeline_wrap"));
    let cases = [
        (json!([{ "id": "a", "frames": 48 }]), "/timelines/0 needs either a duration or both frames and fps"),
        (
            json!([{ "id": "a", "duration": 2, "fps": 24, "frames": 48 }]),
            "/timelines/0 needs either a duration",
        ),
        (json!([{ "id": "a", "fps": 500, "frames": 48 }]), "/timelines/0/fps is 500, expected 1..=240"),
        (json!([{ "id": "a", "fps": 24, "frames": 0 }]), "/timelines/0/frames is 0"),
        (json!([{ "id": "a" }]), "/timelines/0/duration is 0"),
        (
            json!([{ "id": "a", "duration": 2, "wrap": true }]),
            "/timelines/0/wrap applies to timelines that loop with repeat only",
        ),
        (
            json!([{ "id": "a", "fps": 24, "frames": 24, "events": [{ "t": 2, "name": "late" }] }]),
            "/timelines/0/events/0/t is 2, expected 0..=1",
        ),
    ];
    for (timelines, wanted) in cases {
        let found = issues(json!({ "timelines": timelines }));
        assert!(found.iter().any(|f| f.contains(wanted)), "{timelines}: {found:?}");
    }
    let fresh = scene(json!({ "clock": { "seed": null } }));
    assert_eq!((fresh.clock.seed, fresh.features().contains("fresh_seed")), (None, true));
    let written: Json = serde_json::from_str(&fresh.to_json().unwrap()).unwrap();
    assert!(written["clock"]["seed"].is_null() && written["clock"].get("seed").is_some(), "{written}");
    let fixed = scene(json!({ "clock": { "seed": 7 } }));
    let runtime = Runtime::new(std::sync::Arc::new(fixed.clone()), Values::default());
    assert_eq!((runtime.seed, fixed.features().contains("fresh_seed")), (7, false));
    assert_eq!(scene(json!({})).clock.seed, Some(0));
}

#[test]
fn the_scene_reference_lists_every_described_effect_and_parameter() {
    let doc = include_str!("../../../../docs/specs/scene.md");
    let (_, rest) = doc.split_once("### Effects not drawn yet").unwrap();
    let (section, _) = rest.split_once("\n## ").unwrap();
    for d in undrawn() {
        let row = section
            .lines()
            .find(|l| l.starts_with(&format!("| `{}` |", d.id)))
            .unwrap_or_else(|| panic!("{}", d.id));
        assert!(d.params.iter().all(|p| row.contains(&format!("`{}`", p.id))), "{row}");
    }
    for b in builtin() {
        let mut more = newer_params(&b.id).peekable();
        if more.peek().is_none() {
            continue;
        }
        let named = |l: &&str| l.split('|').nth(1).is_some_and(|c| c.contains(&format!("`{}`", b.id)));
        let row = section.lines().find(named).unwrap_or_else(|| panic!("{}", b.id));
        assert!(more.all(|p| row.contains(&format!("`{}`", p.id))), "{row}");
    }
}

#[test]
fn the_table_of_equivalent_effects_names_effects_and_parameters_that_exist() {
    let doc = include_str!("../../../../docs/specs/scene.md");
    let table = doc.split("### Effects with an equivalent").nth(1).unwrap().split("###").next().unwrap();
    let ticks = |cell: &str| cell.split('`').skip(1).step_by(2).map(String::from).collect::<Vec<_>>();
    let rows: Vec<Vec<&str>> = table
        .lines()
        .filter(|l| l.starts_with("| ") && !l.contains("---"))
        .map(|l| l.split('|').collect())
        .skip(1)
        .collect();
    assert!(rows.len() >= 25, "{}", rows.len());
    for row in rows {
        let id = ticks(row[2]).remove(0);
        let params: Vec<String> =
            ticks(row[3]).into_iter().filter(|p| !p.contains(' ') && p != "linear").collect();
        let known: Vec<String> = match id.as_str() {
            "mask" => vec!["texture".into(), "strength".into()],
            _ => {
                described(&id).unwrap_or_else(|| panic!("{id}")).params.iter().map(|p| p.id.clone()).collect()
            }
        };
        assert!(id == "mask" || drawn(&id, None), "{id}");
        params.iter().for_each(|p| assert!(known.contains(p), "{id}.{p}"));
    }
}
