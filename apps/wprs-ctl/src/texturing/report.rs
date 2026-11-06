use crate::show::bytes;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use wprs_editor::pictures;
use wprs_ktx2::TARGETS;

pub(super) fn shown(dir: &std::path::Path, doc: &Value, id: &str) -> Result<Value> {
    let asset = doc.pointer(&format!("/scene/assets/{}", wprs_editor::ops::escape(id)));
    let path = asset.and_then(|a| a["path"].as_str()).ok_or_else(|| anyhow!("there is no asset {id}"))?;
    let file = dir.join(path);
    let size = std::fs::metadata(&file)?.len();
    let (w, h) = pictures::dimensions(&file).map_err(|e| anyhow!("the picture cannot be read: {e}"))?;
    let packed = std::fs::read(&file).ok().and_then(|b| wprs_ktx2::info(&b).ok());
    let mips = asset.is_some_and(|a| a["mipmaps"] != false);
    let gpu = |t: wprs_ktx2::Target| match &packed {
        Some(info) => wprs_ktx2::gpu_bytes(info, t, mips),
        None => u64::from(w) * u64::from(h) * 4,
    };
    let plain = |t| packed.as_ref().is_some_and(|i| wprs_ktx2::plain(i, t));
    let classes =
        TARGETS.map(|t| json!({ "target": t.name(), "class": t.class(), "gpu": gpu(t), "plain": plain(t) }));
    Ok(json!({ "asset": id, "path": path, "bytes": size, "size": [w, h], "codec": packed.map(|i| i.codec),
        "levels": packed.map(|i| i.levels), "hdr": packed.map(|i| i.hdr), "frames": packed.map(|i| i.frames),
        "cube": packed.map(|i| i.cube), "classes": classes }))
}

pub(super) fn told(v: &Value) -> String {
    let classes = v["classes"].as_array().into_iter().flatten().map(|c| {
        let by = match c["drawn"].as_str() {
            Some("gpu") => " (crop drawn by this card)",
            Some("cpu") => " (crop decoded on the processor)",
            _ => "",
        };
        let kept = if c["plain"] == true { " (kept uncompressed)" } else { "" };
        format!(
            "{} {} {}{kept}{by}",
            c["class"].as_str().unwrap_or("?"),
            c["target"].as_str().unwrap_or("?"),
            bytes(c["gpu"].as_u64().unwrap_or(0))
        )
    });
    let shape = match (v["cube"] == true, v["frames"].as_u64().unwrap_or(1)) {
        (true, _) => ", a cube map shown as a panorama".to_string(),
        (_, 2..) => format!(", {} frames in a grid", v["frames"]),
        _ => String::new(),
    };
    let codec = match v["codec"].as_str() {
        Some(c) => format!(", KTX2 {c} with {} levels{shape}", v["levels"]),
        None => String::new(),
    };
    let size = format!("{}x{}", v["size"][0], v["size"][1]);
    let file = bytes(v["bytes"].as_u64().or(v["after"].as_u64()).unwrap_or(0));
    let mut text = format!(
        "{}: {}, {file}, {size}{codec}\n",
        v["asset"].as_str().unwrap_or("?"),
        v["path"].as_str().unwrap_or("?")
    );
    if let Some(before) = v["before"].as_u64() {
        text += &format!(
            "as KTX2 {}: {} before, {} after\n",
            v["codec"].as_str().unwrap_or("?"),
            bytes(before),
            file
        );
    }
    text + "on the graphics card: " + &classes.collect::<Vec<_>>().join(", ")
}
