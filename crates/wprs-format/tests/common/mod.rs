#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use wprs_format::{FileEntry, Kind, MANIFEST, Manifest, SCHEMA};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

pub type Zip = ZipWriter<Cursor<Vec<u8>>>;

pub fn manifest(kind: Kind) -> Manifest {
    let entry = match kind {
        Kind::Image => "image.png",
        Kind::Video => "loop.webm",
        Kind::Scene => "scene.json",
    };
    Manifest {
        schema: SCHEMA.into(),
        id: "org.example.sunset".into(),
        version: "1.0.0".into(),
        title: "Sunset".into(),
        description: "A calm evening sky".into(),
        authors: vec!["Jane Doe".into()],
        license: "CC-BY-4.0".into(),
        kind,
        entry: entry.into(),
        preview: Some("preview.webp".into()),
        tags: vec!["nature".into()],
        properties: BTreeMap::new(),
        presets: Vec::new(),
    }
}

pub fn entry(path: &str, size: u64) -> FileEntry {
    FileEntry { path: path.into(), size, compressed_size: size, symlink: false }
}

pub fn files(manifest: &Manifest) -> Vec<FileEntry> {
    vec![entry(MANIFEST, 300), entry(&manifest.entry, 1000), entry("preview.webp", 100)]
}

pub fn zip(add: impl FnOnce(&mut Zip)) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    add(&mut zip);
    zip.finish().unwrap().into_inner()
}

pub fn put(zip: &mut Zip, name: &str, data: &[u8]) {
    if name.ends_with('/') {
        zip.add_directory(name, SimpleFileOptions::default()).unwrap();
    } else {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
}

pub fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    zip(|z| entries.iter().for_each(|(name, data)| put(z, name, data)))
}

pub fn package(manifest: &Manifest, extra: &[(&str, &[u8])]) -> Vec<u8> {
    let json = manifest.to_json().unwrap();
    let base: [(&str, &[u8]); 3] =
        [(MANIFEST, json.as_bytes()), (&manifest.entry, b"entry"), ("preview.webp", b"preview")];
    archive(&[&base[..], extra].concat())
}

pub fn patch(mut bytes: Vec<u8>, from: &[u8], to: &[u8]) -> Vec<u8> {
    let hits: Vec<usize> =
        bytes.windows(from.len()).enumerate().filter(|(_, w)| *w == from).map(|(i, _)| i).collect();
    hits.into_iter().for_each(|i| bytes[i..i + from.len()].copy_from_slice(to));
    bytes
}
