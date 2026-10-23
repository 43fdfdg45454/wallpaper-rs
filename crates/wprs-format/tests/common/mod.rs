#![allow(dead_code)]

use image::{ImageFormat, RgbImage};
use std::io::{Cursor, Write};
use wprs_format::{
    Content, FileEntry, Kind, MANIFEST, Manifest, Origin, OriginKind, Previews, Rating, SCHEMA, SCHEMA_V1,
};
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
        schema: SCHEMA_V1.into(),
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
        ..Default::default()
    }
}

pub fn v2(kind: Kind) -> Manifest {
    Manifest {
        schema: SCHEMA.into(),
        preview: None,
        previews: Some(Previews { thumbnail: Some("preview.png".into()), animated: None }),
        content: Some(Content { rating: Rating::General, descriptors: Vec::new() }),
        photosensitive: Some(false),
        origin: Some(Origin { kind: OriginKind::Original, derived_from: Vec::new() }),
        ..manifest(kind)
    }
}

pub fn image(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    RgbImage::from_pixel(width, height, [40, 90, 160].into()).write_to(&mut out, format).unwrap();
    out.into_inner()
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
