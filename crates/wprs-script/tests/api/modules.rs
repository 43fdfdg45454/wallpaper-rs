use super::*;

#[test]
fn modules_share_values_and_import_each_other() {
    let files = [
        (
            "scripts/a.js",
            "import { shared } from 'wprs'; import { double } from './lib/b.js'; shared.n = double(21);",
        ),
        ("scripts/lib/b.js", "export const double = (x) => x * 2;"),
        (
            "scripts/c.js",
            r#"import { shared, scene } from "wprs";
            let error = "";
            try { for (let i = 0; i < 300; i++) shared[`k${i}`] = i; } catch (e) { error = e.message; }
            try { shared.k0 = "x".repeat(70000); } catch (e) { error += ` / ${e.message}`; }
            export function update() { scene.layer("a").text = `${shared.n} ${error}`; }"#,
        ),
    ];
    let mut script =
        wprs_script::Script::load(&wprs_script::Setup::plain(&common::sources(&files)), Default::default())
            .unwrap();
    let out = at(&mut script, 0.0).unwrap();
    assert_eq!(
        text(&out, "a"),
        "42 shared holds at most 256 values / a shared value holds at most 65536 items"
    );
}

#[test]
fn console_lines_are_counted_and_cut() {
    let mut script = load(
        r#"export function update({ time }) {
            if (time === 0) for (let i = 0; i < 30; i++) console.log("line", i, { i });
            else console.warn("y".repeat(2000));
        }"#,
    );
    let first = at(&mut script, 0.0).unwrap();
    assert_eq!((first.console.len(), first.console[1].as_str()), (20, "log main.js: line 1 {\"i\":1}"));
    let second = script.update(&Frame { wall: 5.0, ..Frame::at(1.0) }).unwrap();
    assert_eq!((second.console.len(), second.console[0].len()), (1, 512));
    assert!(second.console[0].starts_with("warn main.js: yyy"));
}

#[test]
fn a_failing_module_stops_alone() {
    let files = [
        (
            "bad.js",
            "import { setInterval } from 'wprs'; setInterval(() => {}, 20); export function update() { throw new Error('boom'); }",
        ),
        (
            "good.js",
            "import { scene } from 'wprs'; export function update({ time }) { scene.layer('a').rotation = time; }",
        ),
    ];
    let mut script =
        wprs_script::Script::load(&wprs_script::Setup::plain(&common::sources(&files)), Default::default())
            .unwrap();
    let outs: Vec<_> = (0..4).map(|t| at(&mut script, f64::from(t)).unwrap()).collect();
    assert!(outs[0].console.iter().any(|l| l == "error bad.js: Error: boom"));
    assert_eq!(
        (outs[1].stopped.len(), outs[2].stopped.as_slice()),
        (0, ["bad.js: Error: boom".to_string()].as_slice())
    );
    assert_eq!((outs[3].layers["a"].rotation, outs[3].wake), (Some(3.0), None));
}
