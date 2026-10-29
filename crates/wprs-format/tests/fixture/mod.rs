#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use wprs_format::{
    Added, Content, Descriptor, Fetch, FileFetch, Keypair, Kind, Limits, Manifest, Origin, Previews,
    Published, Rating, SCHEMA, SCHEMA_V1, Settings, Signer, Text, add, build, init, keygen, publish,
    sign_root, sign_source,
};

pub const NOW: i64 = 1_800_000_000;
pub const DAY: i64 = 86_400;
pub const DAWN: &str = "org.example.dawn";
pub const DUSK: &str = "org.example.dusk";

pub struct Repo {
    pub tmp: TempDir,
    pub dir: PathBuf,
    pub data: PathBuf,
    pub root: Keypair,
    pub online: Keypair,
    pub author: Keypair,
}

impl Repo {
    pub fn new() -> Repo {
        Repo::with(|_, _| {})
    }

    pub fn with(edit: impl FnOnce(&Path, &mut Settings)) -> Repo {
        let tmp = tempfile::tempdir().unwrap();
        let (root, online, author) = (key(), key(), key());
        let mut settings = Settings {
            name: "Example wallpapers".into(),
            root_keys: vec![root.public.clone()],
            online_keys: vec![online.public.clone()],
            authors: BTreeMap::from([("studio".into(), vec![author.public.clone()])]),
            ..Settings::default()
        };
        edit(tmp.path(), &mut settings);
        let dir = tmp.path().join("repo");
        init(&dir, &settings, NOW).unwrap();
        sign_root(&dir, &signer(&root)).unwrap();
        Repo { data: tmp.path().join("data"), dir, root, online, author, tmp }
    }

    pub fn url(&self) -> String {
        self.url_at(&self.dir)
    }

    pub fn url_at(&self, dir: &Path) -> String {
        format!("{}?fingerprint={}", file_url(dir), self.root.fingerprint)
    }

    pub fn package(&self, id: &str, version: &str, key: Option<&Keypair>) -> PathBuf {
        let src = self.tmp.path().join(format!("src-{id}-{version}"));
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("manifest.json"), manifest(id, version).to_json().unwrap()).unwrap();
        fs::write(src.join("image.png"), format!("{id} {version}")).unwrap();
        fs::write(src.join("preview.webp"), b"RIFF\0\0\0\0WEBP").unwrap();
        key.into_iter().for_each(|key| drop(sign_source(&src, &signer(key)).unwrap()));
        let out = self.tmp.path().join(format!("{id}-{version}.wpak"));
        build(&src, &out, &Limits::default()).unwrap();
        out
    }

    pub fn add(&self, id: &str, version: &str) -> Added {
        add(&self.dir, &self.package(id, version, Some(&self.author)), Some("studio"), NOW).unwrap()
    }

    pub fn add_scene(&self, id: &str, version: &str) -> Added {
        let src = self.tmp.path().join(format!("scene-{id}-{version}"));
        fs::create_dir_all(&src).unwrap();
        let manifest = Manifest { kind: Kind::Scene, entry: "scene.json".into(), ..manifest(id, version) };
        fs::write(src.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
        let scene = r##"{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32]},
            "layers": [{"id": "fill", "type": "solid", "color": "#336699"}]}"##;
        fs::write(src.join("scene.json"), scene).unwrap();
        fs::write(src.join("preview.webp"), b"RIFF\0\0\0\0WEBP").unwrap();
        sign_source(&src, &signer(&self.author)).unwrap();
        let out = self.tmp.path().join(format!("{id}-{version}.wpak"));
        build(&src, &out, &Limits::default()).unwrap();
        add(&self.dir, &out, Some("studio"), NOW).unwrap()
    }

    pub fn add_v2(&self, id: &str, version: &str) -> Added {
        let src = self.tmp.path().join(format!("v2-{id}-{version}"));
        fs::create_dir_all(&src).unwrap();
        let title =
            BTreeMap::from([("en".to_string(), "Storm".to_string()), ("es".into(), "Tormenta".into())]);
        let manifest = Manifest {
            schema: SCHEMA.into(),
            title: Text::Localized(title),
            default_locale: Some("en".into()),
            changes: Some("- Darker clouds".into()),
            content: Some(Content { rating: Rating::Mature, descriptors: vec![Descriptor::Violence] }),
            photosensitive: Some(true),
            origin: Some(Origin::default()),
            preview: None,
            previews: Some(Previews {
                thumbnail: Some("preview.png".into()),
                animated: Some("loop.gif".into()),
            }),
            ..manifest(id, version)
        };
        fs::write(src.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
        fs::write(src.join("image.png"), picture(image::ImageFormat::Png)).unwrap();
        fs::write(src.join("preview.png"), picture(image::ImageFormat::Png)).unwrap();
        fs::write(src.join("loop.gif"), picture(image::ImageFormat::Gif)).unwrap();
        sign_source(&src, &signer(&self.author)).unwrap();
        let out = self.tmp.path().join(format!("{id}-{version}.wpak"));
        build(&src, &out, &Limits::default()).unwrap();
        add(&self.dir, &out, Some("studio"), NOW).unwrap()
    }

    pub fn add_page(&self, id: &str, version: &str) -> Added {
        let src = self.tmp.path().join(format!("web-{id}-{version}"));
        fs::create_dir_all(&src).unwrap();
        let manifest = Manifest {
            schema: SCHEMA.into(),
            kind: Kind::Web,
            entry: "index.html".into(),
            content: Some(Content { rating: Rating::General, descriptors: Vec::new() }),
            photosensitive: Some(false),
            origin: Some(Origin::default()),
            preview: None,
            previews: Some(Previews { thumbnail: Some("preview.png".into()), animated: None }),
            ..manifest(id, version)
        };
        let mut json = serde_json::to_value(manifest).unwrap();
        json["requires"] = serde_json::json!({ "features": ["web"] });
        fs::write(src.join("manifest.json"), json.to_string()).unwrap();
        fs::write(src.join("index.html"), "<!doctype html>").unwrap();
        fs::write(src.join("preview.png"), picture(image::ImageFormat::Png)).unwrap();
        sign_source(&src, &signer(&self.author)).unwrap();
        let out = self.tmp.path().join(format!("{id}-{version}.wpak"));
        build(&src, &out, &Limits::default()).unwrap();
        add(&self.dir, &out, Some("studio"), NOW).unwrap()
    }

    pub fn publish(&self, now: i64) -> Published {
        publish(&self.dir, &signer(&self.online), 7, now).unwrap()
    }

    pub fn copy(&self, name: &str) -> PathBuf {
        let to = self.tmp.path().join(name);
        copy_dir(&self.dir, &to);
        to
    }
}

fn picture(format: image::ImageFormat) -> Vec<u8> {
    let mut out = io::Cursor::new(Vec::new());
    image::RgbImage::from_pixel(64, 36, [20, 30, 40].into()).write_to(&mut out, format).unwrap();
    out.into_inner()
}

pub fn key() -> Keypair {
    keygen(None).unwrap()
}

pub fn signer(key: &Keypair) -> Signer {
    Signer::new(&key.secret, None).unwrap()
}

pub fn pair(key: &Keypair) -> (&str, &str) {
    (&key.fingerprint, key.public.lines().nth(1).unwrap())
}

pub fn file_url(dir: &Path) -> String {
    format!("file://{}/", dir.display())
}

pub fn manifest(id: &str, version: &str) -> Manifest {
    Manifest {
        schema: SCHEMA_V1.into(),
        id: id.into(),
        version: version.into(),
        title: Text::Plain(format!("Sky at {}", id.rsplit('.').next().unwrap())),
        description: "A calm gradient".into(),
        authors: vec!["Example Studio".into()],
        license: "CC0-1.0".into(),
        kind: Kind::Image,
        entry: "image.png".into(),
        preview: Some("preview.webp".into()),
        tags: vec!["sky".into()],
        ..Default::default()
    }
}

pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for item in fs::read_dir(from).unwrap() {
        let item = item.unwrap();
        let target = to.join(item.file_name());
        if item.file_type().unwrap().is_dir() {
            copy_dir(&item.path(), &target);
        } else {
            fs::copy(item.path(), target).unwrap();
        }
    }
}

pub fn files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        fs::read_dir(dir).unwrap().map(|i| i.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

#[derive(Default)]
pub struct Counting(pub RefCell<Vec<String>>);

impl Fetch for Counting {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        self.0.borrow_mut().push(path.into());
        FileFetch.get(base, path, max)
    }
}

impl Counting {
    pub fn count(&self, prefix: &str) -> usize {
        self.0.borrow().iter().filter(|p| p.starts_with(prefix)).count()
    }
}
