use super::{extension, folder};
use crate::{Error, store};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use wprs_format::Package;
use wprs_scene::Scene;

pub(crate) type Sound = (String, PathBuf, f32);

pub(super) fn extract(
    package: &mut Package<BufReader<File>>,
    scene: &Scene,
    path: &Path,
) -> Result<Vec<Sound>, Error> {
    let mut sounds = Vec::new();
    for (id, asset) in &scene.assets {
        if !scene.sounds().any(|s| s.clips().any(|c| c == id)) {
            continue;
        }
        let file = folder(path)?.join(format!("{id}.{}", extension(&asset.path)));
        if !file.is_file() {
            fs::create_dir_all(folder(path)?)?;
            store::write(&file, &package.read_file(&asset.path)?)?;
        }
        let seconds = wprs_scene::Clip::file(&file).map_or(0.0, |clip| clip.seconds);
        sounds.push((id.clone(), file, seconds));
    }
    Ok(sounds)
}
