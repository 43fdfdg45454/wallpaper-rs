use std::path::Path;
use std::{env, fs};

fn main() {
    let dir = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join("tests/effects");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok()?.strip_suffix(".rs").map(String::from))
        .filter(|name| name != "mod")
        .collect();
    names.sort();
    let mods: String =
        names.iter().map(|n| format!("#[path = {:?}]\nmod {n};\n", dir.join(format!("{n}.rs")))).collect();
    let out = Path::new(&env::var("OUT_DIR").expect("set by cargo")).join("effect_tests.rs");
    fs::write(out, mods).expect("a writable OUT_DIR");
}
