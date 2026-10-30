use serde_json::json;
mod common;

use wprs_scene::{ParticleCommand, ScriptOutputs};
use wprs_script::{Frame, Script};

fn update(script: &mut Script) -> ScriptOutputs {
    script.update(&Frame { dt: 0.016, ..Frame::at(1.0) }).unwrap()
}

#[test]
fn scripts_play_emit_and_tune_particles() {
    let source = r#"
        import { particles } from "wprs";
        let n = 0;
        export function update() {
            const sparks = particles("sparks");
            n += 1;
            if (n === 1) { sparks.pause(); sparks.emit(12); sparks.instance.alpha = 0.5; sparks.point(2, [10, 20]); }
            if (n === 2) { sparks.play(); sparks.instance.color = [1, 0, 0, 1]; }
            if (n === 3) { sparks.stop(); return sparks.playing; }
            return sparks.playing;
        }
    "#;
    let mut script = common::load(source);
    let first = update(&mut script);
    let sparks = &first.particles["sparks"];
    assert_eq!(sparks.commands, [ParticleCommand::Pause, ParticleCommand::Emit(12)]);
    assert_eq!((sparks.instance.alpha, sparks.points.get(&2)), (Some(0.5), Some(&[10.0, 20.0])));
    assert_eq!(first.values["main.js"], json!(false));
    let second = update(&mut script);
    let sparks = &second.particles["sparks"];
    assert_eq!(sparks.commands, [ParticleCommand::Play]);
    assert_eq!((sparks.instance.alpha, sparks.instance.color), (Some(0.5), Some([1.0, 0.0, 0.0, 1.0])));
    let third = update(&mut script);
    assert_eq!(
        (third.particles["sparks"].commands.clone(), third.values["main.js"].clone()),
        (vec![ParticleCommand::Stop], json!(false))
    );
    assert_eq!(update(&mut script).particles["sparks"].commands, []);
}

#[test]
fn particle_calls_are_bounded() {
    for (call, message) in [
        ("particles('a').emit(1001)", "emit takes 1 to 1000"),
        ("particles('a').emit(0)", "emit takes 1 to 1000"),
        ("particles('a').point(8, [0, 0])", "point takes 0 to 7"),
        ("particles('a').instance.glow = 1", "glow is not an instance field"),
    ] {
        let source = format!("import {{ particles }} from \"wprs\";\nexport function update() {{ {call}; }}");
        let out = update(&mut common::load(&source));
        let error = out.console.join("\n");
        assert!(error.contains(message), "{call}: {error}");
    }
}
