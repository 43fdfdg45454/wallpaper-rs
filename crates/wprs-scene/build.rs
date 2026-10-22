use std::path::Path;
use std::{env, fs};

fn main() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join("effects");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut ids: Vec<String> = fs::read_dir(&dir)
        .expect("the effects directory")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            name.strip_suffix(".json").map(String::from)
        })
        .collect();
    ids.sort();
    let files: String = ids
        .iter()
        .map(|id| {
            let (json, wgsl) = (dir.join(format!("{id}.json")), dir.join(format!("{id}.wgsl")));
            format!("({id:?}, include_str!({json:?}), include_str!({wgsl:?})),\n")
        })
        .collect();
    let out = Path::new(&env::var("OUT_DIR").expect("set by cargo")).join("effects.rs");
    fs::write(out, format!("[\n{files}]\n")).expect("a writable OUT_DIR");
}
