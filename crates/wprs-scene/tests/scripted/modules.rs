use super::*;

#[test]
fn scripts_declare_their_inputs_by_what_they_import() {
    let source = r#"
        // import { audio } from "wprs";
        const text = "import { input } from 'wprs'";
        import { scene, particles as p, setTimeout } from "wprs";
        import * as math from "wprs/math";
    "#;
    assert_eq!(imported(source), [ScriptApi::Timers, ScriptApi::Particles, ScriptApi::Math]);
    assert_eq!(imported("import * as w from 'wprs'"), [ScriptApi::Audio, ScriptApi::Pointer]);
    assert_eq!(imported("import { f } from './lib.js'"), [ScriptApi::Shared]);
    assert_eq!(imported("import './lib.js'; import('./x.js');"), [ScriptApi::Shared]);
    assert_eq!(
        imported("import { audio as a, input } from \"wprs\""),
        [ScriptApi::Audio, ScriptApi::Pointer]
    );
    let scene = Scene::from_json(
        &json!({ "schema": SCHEMA, "canvas": { "size": [10, 10] },
        "scripts": [{ "path": "a.js" }, { "path": "b.js", "uses": ["timers"] }] })
        .to_string(),
    )
    .unwrap();
    let files = [
        ("a.js".to_string(), "/* audio cursor */ export const x = 1;".to_string()),
        ("b.js".into(), "import { audio, input } from 'wprs';".into()),
    ];
    assert_eq!(script_inputs(&scene, &files), [false, false]);
    let files = [("a.js".to_string(), "import { audio } from 'wprs';".to_string())];
    assert_eq!(script_inputs(&scene, &files), [true, false]);
}

#[test]
fn modules_that_declare_nothing_are_held_to_the_first_interface() {
    let first = r#"import { scene, audio as a } from "wprs";
        // import { shared } from "wprs"; setTimeout(f);
        const text = "console.log";
        export function update(f) { return f.console + other.setTimeout; }
        export const helper = 1;"#;
    assert!(beyond_first(first).is_empty(), "{:?}", beyond_first(first));
    let newer = r#"import { shared, console as c } from "wprs";
        import { Vec2 } from "wprs/math";
        import "./lib.js";
        export { x } from "./x.js";
        export async function init() { setTimeout(() => {}, 1); }
        const resize = 1;
        export { resize as settings };"#;
    assert_eq!(
        beyond_first(newer),
        [
            "console",
            "imports of ./x.js",
            "imports of other modules",
            "imports of wprs/math",
            "its init export",
            "its settings export",
            "setTimeout",
            "shared"
        ]
    );
    let scene = Scene::from_json(
        &json!({ "schema": SCHEMA, "canvas": { "size": [10, 10] },
            "scripts": [{ "path": "a.js" }, { "path": "b.js", "uses": ["timers"] }, { "path": "c.js" }],
            "layers": [{ "id": "l", "type": "solid", "color": "#ffffff", "opacity": { "script": "c.js", "fn": "f" } }] })
        .to_string(),
    )
    .unwrap();
    assert_eq!(scene.first_interface(), ["a.js"]);
    let read =
        |text: &'static str| move |path: &str| Some(if path == "a.js" { text } else { newer }.to_owned());
    assert_eq!(scene.undeclared(read(first)), None);
    let (at, message) = scene.undeclared(read("import { shared } from 'wprs';")).unwrap();
    assert_eq!(
        (at, message.as_str()),
        (0, "a.js declares nothing in scene.json but uses shared; declare its events and uses")
    );
}
