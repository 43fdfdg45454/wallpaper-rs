use super::{IMAGES, Preview, decode, folder, open};
use crate::{Error, said, store};
use image::RgbaImage;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use wprs_format::{Kind, Package};
use wprs_render::SceneAssets;
use wprs_scene::Scene;

const POSTER: u32 = 512;

pub(crate) fn check(path: &Path) -> Result<Preview, Error> {
    let mut package = open(path)?;
    let thumbnail = package.manifest.thumbnail().cloned().map(|p| package.read_file(&p)).transpose()?;
    let thumbnail = thumbnail.and_then(|bytes| decode(bytes).ok());
    match package.manifest.kind {
        Kind::Scene => {
            let scene = package.scene()?;
            let [width, height] = scene.canvas.size;
            Ok((thumbnail.or_else(|| drawn(&mut package, &scene)), Some((width, height))))
        }
        Kind::Image => {
            let picture = decode(package.read_file(&package.manifest.entry.clone())?)?;
            let size = Some(picture.dimensions());
            Ok((Some(thumbnail.unwrap_or(picture)), size))
        }
        _ => Ok((thumbnail, None)),
    }
}

pub(crate) fn media(item: &wprs_library::Item) -> Result<(PathBuf, &'static str), Error> {
    let kind = match (item.kind, item.package.as_ref().map(|p| p.kind.as_str())) {
        (wprs_library::Kind::Video, _) | (_, Some("video")) => "video",
        (wprs_library::Kind::Package, Some("image")) | (wprs_library::Kind::Image, _) => "image",
        _ => "scene",
    };
    match item.kind == wprs_library::Kind::Package && kind != "scene" {
        true => Ok((entry(&item.path)?, kind)),
        false => Ok((item.path.clone(), kind)),
    }
}

pub(crate) fn entry(path: &Path) -> Result<PathBuf, Error> {
    let entry = |p: &PathBuf| p.file_stem().is_some_and(|stem| stem == "entry");
    let kept = fs::read_dir(folder(path)?).ok().and_then(|d| d.flatten().map(|e| e.path()).find(entry));
    if let Some(kept) = kept {
        return Ok(kept);
    }
    let mut package = open(path)?;
    let name = package.manifest.entry.clone();
    let kind = super::extension(&name);
    let picture = package.manifest.kind == Kind::Image && IMAGES.contains(&kind.as_str());
    if !picture && !(package.manifest.kind == Kind::Video && super::VIDEOS.contains(&kind.as_str())) {
        let title = package.manifest.title_in(&[]);
        return Err(said("android.not_a_file", "%1 is not a picture or video package", &[&title]));
    }
    let file = folder(path)?.join(format!("entry.{kind}"));
    fs::create_dir_all(folder(path)?)?;
    let mut part = tempfile::NamedTempFile::new_in(folder(path)?)?;
    package.copy_file(&name, part.as_file_mut())?;
    part.as_file().sync_all()?;
    Ok(part.persist(&file).map(|_| file).map_err(|e| e.error)?)
}

fn drawn(package: &mut Package<BufReader<fs::File>>, scene: &Scene) -> Option<RgbaImage> {
    let assets = SceneAssets::read(scene, |path| package.read_file(path).map_err(|e| e.to_string())).ok()?;
    let props = wprs_scene::effective(&package.manifest.properties, &[]);
    let clock =
        (crate::stage::report::wall(), package.manifest.default_locale.as_deref().unwrap_or_default());
    wprs_render::Renderer::shared().ok()?.poster(scene, &assets, &props, clock, POSTER).ok()
}

pub(crate) fn about(path: &Path, languages: &[String]) -> Option<wprs_core::model::About> {
    let mut about = wprs_format::peek(path).ok()?.about(languages);
    if about.schema != wprs_format::SCHEMA_V1 {
        about.description = wprs_core::markup::plain(&about.description);
        about.changes = wprs_core::markup::plain(&about.changes);
    }
    Some(about)
}

pub(crate) fn lacks(path: &Path, facets: &[String]) -> Vec<String> {
    let listed = wprs_format::peek(path).map(|m| m.features().to_vec()).unwrap_or_default();
    let mut found = wprs_format::features::lacks("android", listed.iter().chain(facets));
    found.sort();
    found.dedup();
    found
}

pub(crate) fn examples(dir: &Path) -> Result<Vec<String>, Error> {
    let mut sources: Vec<PathBuf> = fs::read_dir(dir)?.flatten().map(|e| e.path()).collect();
    sources.retain(|source| source.join(wprs_format::MANIFEST).is_file());
    sources.sort();
    let built = tempfile::tempdir_in(store::dir()?)?;
    let mut added = Vec::new();
    for source in sources {
        let name = source.file_name().and_then(|n| n.to_str()).unwrap_or("scene").to_owned();
        let wpak = built.path().join(format!("{name}.wpak"));
        wprs_format::build(&source, &wpak, &wprs_format::Limits::default())?;
        added.push(store::store(&mut fs::File::open(&wpak)?, &name)?.0);
    }
    Ok(added)
}

pub(crate) fn animated(uid: &str, path: &Path) -> Option<String> {
    let manifest = wprs_format::peek(path).ok()?;
    let name = manifest.animated()?;
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    (ext == "gif" || ext == "webp").then_some(())?;
    let file = store::thumb_file(uid, &format!("animated.{ext}")).ok()?;
    if !file.is_file() {
        let data = wprs_format::open(path, &wprs_format::Limits::default()).ok()?.read_file(name).ok()?;
        let part = file.with_extension("part");
        fs::write(&part, data).and_then(|()| fs::rename(&part, &file)).ok()?;
    }
    Some(file.to_string_lossy().into_owned())
}
