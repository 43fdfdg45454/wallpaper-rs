use super::{NOW, Repo, copy_dir, file_url, key, manifest, picture, signer};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use wprs_format::{
    Added, Content, Descriptor, Keypair, Kind, Limits, Manifest, Origin, Previews, Published, Rating, SCHEMA,
    Settings, Text, add, build, init, publish, sign_root, sign_source,
};

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
