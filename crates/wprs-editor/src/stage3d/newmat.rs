use crate::ops::{Op, escape};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::message::said;

pub const TEMPLATE: &str = concat!(
    "fn material(s: Surface) -> Surface {\n",
    "    var o = s;\n",
    "    let tint = param(0u);\n",
    "    o.albedo = vec4<f32>(o.albedo.rgb * tint.rgb, o.albedo.a);\n",
    "    o.emissive = o.emissive + o.albedo.rgb * param(1u).x * (0.5 + 0.5 * sin(frame.time));\n",
    "    return o;\n",
    "}\n",
);

fn descriptor(name: &str) -> String {
    let v = json!({ "id": name, "stage": "material", "cost": {},
        "params": [{ "id": "tint", "type": "color", "default": "#ffffff" },
                   { "id": "glow", "type": "float", "default": 0.0, "min": 0, "max": 4 }] });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

pub(crate) fn create(dir: &Path, doc: &Value, name: &str) -> Result<Vec<Op>> {
    crate::clips::valid(name)?;
    ensure!(
        doc.pointer(&format!("/scene/effects/{}", escape(name))).is_none() && !wprs_scene::reserved(name),
        said("editor.material_taken", "there is already a shader named %1", &[&name])
    );
    std::fs::create_dir_all(dir.join("effects"))?;
    let (json, wgsl) = (dir.join(format!("effects/{name}.json")), dir.join(format!("effects/{name}.wgsl")));
    for (path, body) in [(&json, descriptor(name)), (&wgsl, TEMPLATE.to_owned())] {
        if !path.exists() {
            std::fs::write(path, body)?;
        }
    }
    crate::shaders::import(dir, doc, &json, Some(&wgsl))
}

fn located(error: &str) -> Option<(usize, usize)> {
    let at = error.find("wgsl:")? + 5;
    let mut parts = error[at..].split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty());
    let line: usize = parts.next()?.parse().ok()?;
    let column = parts.next().and_then(|c| c.parse().ok()).unwrap_or(1);
    Some((wprs_scene::shader_line(line)?, column))
}

pub fn check(dir: &Path, doc: &Value, name: &str) -> Value {
    let entry = doc.pointer(&format!("/scene/effects/{}", escape(name)));
    let read =
        |k: &str| entry.and_then(|e| e[k].as_str()).and_then(|p| std::fs::read_to_string(dir.join(p)).ok());
    let (Some(json), Some(wgsl)) = (read("descriptor"), read("wgsl")) else {
        return json!({ "name": name, "ok": false, "error": "the shader files are missing" });
    };
    let stage = serde_json::from_str::<Value>(&json).ok().and_then(|v| v["stage"].as_str().map(String::from));
    let result = wprs_scene::Descriptor::packaged(name, &json, &wgsl);
    let error = result.as_ref().err().cloned();
    let at = error.as_deref().and_then(located);
    json!({ "name": name, "ok": error.is_none(), "material": stage.as_deref() == Some("material"),
        "error": error, "line": at.map(|a| a.0), "column": at.map(|a| a.1), "source": wgsl })
}
