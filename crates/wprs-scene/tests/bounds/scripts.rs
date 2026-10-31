use super::*;

#[test]
fn scripts_media_and_pointer_hooks_name_their_broken_fields() {
    let script = |n: usize| json!({ "path": format!("scripts/{n}.js") });
    for (changes, fault) in [
        (vec![("/scripts/0/events", json!(["click", "click"]))], "/scripts/0/events repeats an id"),
        (vec![("/scripts/0/events", json!(["shake"]))], "unknown variant `shake`"),
        (vec![("/scripts/0/uses", json!(["network"]))], "unknown variant `network`"),
        (
            vec![("/scripts/0/params/speed/default", json!(20))],
            "/scripts/0/params/speed/default is 20, expected 0..=10",
        ),
        (vec![("/scripts/0/params/tint/default", json!(1))], "/scripts/0/params/tint/default does not fit"),
        (vec![("/scripts", (0..65).map(script).collect())], "/scripts has 65 entries, at most 64"),
        (vec![("/pools", json!({}))], "/scripts/0/uses does not fit"),
        (
            vec![("/layers/0/solid", json!(false)), ("/layers/0/hit", json!([])), ("/timelines", json!([]))],
            "/scripts/0/events does not fit",
        ),
        (
            vec![("/layers/4/opacity/params", json!({ "speed": "fast" }))],
            "/layers/4/opacity/params/speed does not fit",
        ),
        (
            vec![("/layers/4/opacity/params", json!({ "pace": 1 }))],
            "names the unknown script parameter \"pace\"",
        ),
        (
            vec![("/layers/4/opacity/params/speed", json!(11))],
            "/layers/4/opacity/params/speed is 11, expected 0..=10",
        ),
        (vec![("/layers/4/opacity/fn", json!("1go"))], "/layers/4/opacity/fn does not fit"),
        (
            vec![("/layers/4/opacity", json!({ "keys": [[0, 1]], "fn": "x" }))],
            "/layers/4/opacity does not fit",
        ),
        (
            vec![("/layers/2/layers/2/color", json!({ "media": "title" }))],
            "/layers/2/layers/2/color/media does not fit",
        ),
        (
            vec![("/layers/2/layers/2/text", json!({ "media": "progress" }))],
            "/layers/2/layers/2/text/media does not fit",
        ),
        (vec![("/layers/2/layers/2/text", json!({ "media": "lyrics" }))], "did not match any variant"),
        (vec![("/timelines/2/trigger", json!("media:skip"))], "names the unknown media trigger \"skip\""),
        (
            vec![("/timelines/0/trigger", json!("click:show"))],
            "names the unknown layer that takes clicks \"show\"",
        ),
        (
            vec![("/timelines/1/trigger", json!("leave:badge"))],
            "names the unknown layer that takes the pointer \"badge\"",
        ),
        (vec![("/timelines/3/trigger", json!("event:jump"))], "names the unknown event \"jump\""),
        (
            vec![("/templates/fox", json!({ "id": "fox", "type": "solid", "color": "#000000" }))],
            "/templates/fox repeats an id",
        ),
        (vec![("/templates/bar/opacity", json!(2))], "/templates/bar/opacity is 2, expected 0..=1"),
        (vec![("/pools/waves/max", json!(1000))], "/pools/waves/max is 1000, expected 1..=256"),
        (vec![("/pools/waves/max_vertices", json!(2))], "/pools/waves/max_vertices is 2, expected 3..=65536"),
        (vec![("/layers/0/solid", json!({ "alpha": 2 }))], "/layers/0/solid/alpha is 2, expected 0..=1"),
        (
            vec![(
                "/layers/4/effects",
                json!([{ "effect": "blur", "name": "soft" }, { "effect": "blur", "name": "soft" }]),
            )],
            "/layers/4/effects/1/name repeats an id",
        ),
        (vec![("/layers/4/effects/0/name", json!("Soft Glow"))], "/layers/4/effects/0/name must match"),
        (vec![("/assets/cover/path", json!("song.mp3"))], "/assets/cover/media does not fit"),
    ] {
        expect(&changes, fault);
    }
}
