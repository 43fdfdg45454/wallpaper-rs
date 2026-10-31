use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let mut langs: Vec<_> = fs::read_dir("../../po").unwrap().map(|e| e.unwrap().path()).collect();
    langs.sort();
    let catalogs: String =
        (langs.iter().map(|dir| (dir, dir.join("wprs-messages.po"))).filter(|(_, po)| po.is_file()))
            .map(|(dir, po)| {
                format!("({:?}, include_str!({:?})),", dir.file_name().unwrap(), po.canonicalize().unwrap())
            })
            .collect();
    fs::write(PathBuf::from(env::var("OUT_DIR").unwrap()).join("catalogs.rs"), format!("&[{catalogs}]"))
        .unwrap();
    println!("cargo::rerun-if-changed=../../po");
}
