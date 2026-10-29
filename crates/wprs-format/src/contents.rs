use crate::validate::{extension, within};
use crate::{Error, FileEntry, PresetFile, Result, ensure, media, require};
use std::collections::BTreeSet;
use wprs_scene::{
    Clip, MAX_SOUND_BYTES, MAX_SOUND_SECONDS, MAX_SOUND_TOTAL, MAX_STREAM_BYTES, MAX_STREAM_SECONDS,
    MAX_STREAM_TOTAL, Scene,
};

pub(crate) fn size(entries: &[FileEntry], path: &str) -> u64 {
    entries.iter().find(|e| e.path == path).map_or(0, |e| e.size)
}

pub(crate) fn preset_files(preset: &PresetFile, entries: &[FileEntry]) -> Result<()> {
    for (id, path) in &preset.files {
        ensure(entries.iter().any(|e| e.path == *path), || Error::Missing(path.clone()))?;
        let media = extension(path).is_some_and(|e| media::shown(&e));
        require(media, &format!("preset.files.{id}"), "expected a picture or a video of the package")?;
    }
    Ok(())
}

pub(crate) fn binaries(entries: &[FileEntry], mut head: impl FnMut(&str) -> Result<Vec<u8>>) -> Result<()> {
    for path in entries.iter().map(|e| &e.path).filter(|p| p.ends_with(".wasm")) {
        require(head(path)?.starts_with(b"\0asm"), path, "is not a WebAssembly module")?;
    }
    Ok(())
}

pub(crate) const SOUND_EDGE: u64 = 1 << 20;

pub(crate) type Ends = (Vec<u8>, Vec<u8>);

pub(crate) fn sounds(
    scene: &Scene,
    entries: &[FileEntry],
    mut read: impl FnMut(&str, Option<u64>) -> Result<Ends>,
) -> Result<()> {
    let clips = scene.sounds().flat_map(|s| s.clips().map(move |id| (id, s.stream)));
    let paths: BTreeSet<(&str, bool)> =
        clips.filter_map(|(id, stream)| Some((scene.assets.get(id)?.path.as_str(), stream))).collect();
    let streamed = paths.iter().filter(|(_, stream)| *stream).map(|(path, _)| size(entries, path)).sum();
    within("size of the streamed sounds", streamed, MAX_STREAM_TOTAL)?;
    let mut total = 0.0;
    for (path, stream) in paths {
        let size = size(entries, path);
        let (bytes, seconds) = if stream {
            (MAX_STREAM_BYTES, MAX_STREAM_SECONDS)
        } else {
            (MAX_SOUND_BYTES, MAX_SOUND_SECONDS)
        };
        within("sound size", size, bytes)?;
        let (head, tail) = read(path, stream.then_some(SOUND_EDGE))?;
        let tail = if tail.is_empty() { &head } else { &tail };
        let clip = Clip::sample(path, &head, tail, size)
            .map_err(|reason| Error::Invalid { field: path.into(), reason })?;
        let reason = format!("lasts {:.1} s, at most {seconds} s are allowed", clip.seconds);
        require(clip.seconds <= seconds, path, &reason)?;
        if !stream {
            total += clip.seconds;
            within("length of the sounds in seconds", total.ceil() as u64, MAX_SOUND_TOTAL as u64)?;
        }
    }
    Ok(())
}
