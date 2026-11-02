#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        type Shelf = super::ShelfRust;

        #[qinvokable]
        fn parts(self: &Shelf, package: &QString) -> QString;
        #[qinvokable]
        fn offer(self: &Shelf, folder: &QString, args: &QString) -> QString;
    }
}

use crate::scenefile::Shown;
use cxx_qt_lib::QString;
use qobject::Shelf;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wprs_editor::offer::{Offer, make};
use wprs_editor::shelf::Pick;
use wprs_editor::{Project, stock};

#[derive(Default)]
pub struct ShelfRust;

#[derive(Deserialize)]
struct Asked {
    out: PathBuf,
    parts: Vec<String>,
    id: String,
    title: String,
    license: String,
    attribution: String,
    #[serde(default)]
    thumbnail: Option<PathBuf>,
    #[serde(default)]
    thumbnails: BTreeMap<String, PathBuf>,
    #[serde(default)]
    titles: BTreeMap<String, String>,
}

fn answer(result: anyhow::Result<Value>) -> QString {
    let value = result.unwrap_or_else(|e| json!({ "error": e.shown() }));
    QString::from(&value.to_string())
}

fn offered(folder: &Path, asked: Asked) -> anyhow::Result<Value> {
    let picks = asked.parts.iter().map(|p| Pick::parse(p)).collect::<anyhow::Result<Vec<_>>>()?;
    let source = Project::open(folder)?;
    anyhow::ensure!(!source.dirty(), "save the scene first");
    let thumbnails = asked.thumbnails.into_iter().filter(|(_, f)| !f.as_os_str().is_empty()).collect();
    let offer = Offer {
        id: asked.id,
        title: asked.title,
        license: asked.license,
        attribution: asked.attribution,
        thumbnail: asked.thumbnail.filter(|f| !f.as_os_str().is_empty()),
        thumbnails,
        titles: asked.titles,
    };
    let made = make(&source.dir, &source.doc, &picks, &asked.out, &offer)?;
    Ok(json!({ "folder": made.dir }))
}

impl Shelf {
    pub fn parts(&self, package: &QString) -> QString {
        let cache = dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("wallpaper-rs").join("parts");
        answer(stock::browse(Path::new(&package.to_string()), Some(&cache)))
    }

    pub fn offer(&self, folder: &QString, args: &QString) -> QString {
        let asked = serde_json::from_str::<Asked>(&args.to_string()).map_err(anyhow::Error::from);
        answer(asked.and_then(|a| offered(Path::new(&folder.to_string()), a)))
    }
}
