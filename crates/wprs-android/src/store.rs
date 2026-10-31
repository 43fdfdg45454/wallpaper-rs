use crate::{Error, library, media, pack, settings};
use image::{DynamicImage, RgbaImage};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use wprs_library::{Item, Kind, Library, Meta, qhash};
use wprs_render::{decode_image, image_size, palette, thumbnails};

const MEDIA: &str = "media";
const THUMBS: &str = "thumbs";
const EDGE: u32 = 512;
const DARK_TEXT: i32 = 1;
const DARK_THEME: i32 = 2;
const OLD: [&str; 4] = ["wallpaper", "pending", "wallpaper.colors", "pending.colors"];

static FILES: OnceLock<PathBuf> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(1);

pub(crate) fn prepare(files: &str) -> Result<(), Error> {
    if FILES.set(files.into()).is_err() {
        return Ok(());
    }
    let dir = dir()?;
    [MEDIA, THUMBS].iter().try_for_each(|name| fs::create_dir_all(dir.join(name)))?;
    wprs_scene::keep_strikes(&dir.join("stopped-effects"));
    settings::prepare()?;
    if let Ok((uid, _)) =
        File::open(dir.join(OLD[0])).map_err(Error::from).and_then(|mut f| store(&mut f, ""))
    {
        let path = library::with(|lib| Ok(lib.item(&uid)?.path))?;
        settings::update(|config| {
            config.set_image("", path, None);
            Ok(())
        })?;
    }
    OLD.iter().for_each(|name| drop(fs::remove_file(dir.join(name))));
    Ok(())
}

pub(crate) fn dir() -> Result<&'static Path, Error> {
    FILES.get().map(PathBuf::as_path).ok_or(Error::Unprepared)
}

pub(crate) fn generation() -> u64 {
    GENERATION.load(Relaxed)
}

pub(crate) fn changed() {
    GENERATION.fetch_add(1, Relaxed);
}

pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut file = tempfile::NamedTempFile::new_in(dir()?)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    Ok(file.persist(path).map(drop).map_err(|e| e.error)?)
}

pub(crate) fn thumb_file(uid: &str, extension: &str) -> Result<PathBuf, Error> {
    Ok(dir()?.join(THUMBS).join(format!("{uid}.{extension}")))
}

pub(crate) fn store(source: &mut impl Read, name: &str) -> Result<(String, Kind), Error> {
    let media = dir()?.join(MEDIA);
    let mut copy = tempfile::NamedTempFile::new_in(&media)?;
    std::io::copy(source, &mut copy)?;
    let unreadable = || Error::Unreadable(name.into());
    let extension = media::extension(copy.path()).ok_or_else(unreadable)?;
    let kind = match extension {
        "wpak" => Kind::Package,
        _ if media::is_video(copy.path()) => Kind::Video,
        _ => Kind::Image,
    };
    let (small, size) = match kind {
        Kind::Video => (None, None),
        Kind::Package => pack::check(copy.path())?,
        Kind::Image => {
            let small = decode_image(copy.path(), EDGE).map_err(|_| unreadable())?;
            (Some(small), image_size(copy.path()).ok())
        }
    };
    copy.as_file().sync_all()?;
    let hash: String = qhash(copy.path())?[..8].iter().map(|b| format!("{b:02x}")).collect();
    let path = media.join(format!("{hash}.{extension}"));
    let title =
        Path::new(name).file_stem().and_then(|stem| stem.to_str()).unwrap_or_default().trim().to_owned();
    library::with(|lib| {
        let known = path.to_str().filter(|_| path.is_file()).and_then(|text| lib.resolve_item(text).ok());
        let uid = match known {
            Some(uid) => uid,
            None => {
                copy.persist(&path).map_err(|e| e.error)?;
                let uid = lib.import_file(&path, library::now())?;
                if !title.is_empty() && kind != Kind::Package {
                    let _ = lib.set_title(&uid, &title);
                }
                match small {
                    Some(small) => describe(lib, &uid, DynamicImage::ImageRgba8(small), size, None)?,
                    None if kind == Kind::Package => {
                        let (width, height) = (size.map(|s| s.0), size.map(|s| s.1));
                        lib.set_meta(&uid, &Meta { width, height, ..Meta::default() })?;
                    }
                    None => {}
                }
                uid
            }
        };
        Ok((uid, kind))
    })
}

pub(crate) fn poster_source(uid: &str) -> Result<Option<PathBuf>, Error> {
    let item = library::with(|lib| Ok(lib.item(uid)?))?;
    let film = item.package.as_ref().is_some_and(|p| p.kind == "video") && !thumb_file(uid, "jpg")?.is_file();
    film.then(|| pack::entry(&item.path)).transpose()
}

pub(crate) fn poster(
    uid: &str,
    argb: &[i32],
    width: u32,
    size: (u32, u32),
    clip: (u32, bool),
) -> Result<(), Error> {
    let rgba = argb.iter().flat_map(|c| c.rotate_left(8).to_be_bytes()).collect();
    let height = u32::try_from(argb.len()).unwrap_or(0).checked_div(width).unwrap_or(0);
    let frame = RgbaImage::from_raw(width, height, rgba).filter(|_| height > 0);
    let frame = DynamicImage::ImageRgba8(frame.ok_or_else(|| Error::Unreadable("empty frame".into()))?);
    library::with(|lib| describe(lib, uid, frame, Some(size), Some(clip)))
}

fn describe(
    lib: &mut Library,
    uid: &str,
    image: DynamicImage,
    size: Option<(u32, u32)>,
    clip: Option<(u32, bool)>,
) -> Result<(), Error> {
    let palette = palette(&image);
    let mut colors = [0; 4];
    palette.colors.iter().zip(&mut colors).for_each(|(rgb, slot)| *slot = (0xff00_0000 | rgb) as i32);
    colors[3] = (DARK_TEXT * i32::from(palette.dark_text)) | (DARK_THEME * i32::from(palette.dark_theme));
    write(&thumb_file(uid, "colors")?, colors.map(|c| format!("{c:08x}")).join(" ").as_bytes())?;
    thumbnails(image, &[(EDGE, thumb_file(uid, "jpg")?)], &mut Vec::new())?;
    let (width, height) = (size.map(|s| s.0), size.map(|s| s.1));
    let (duration_ms, audio) = (clip.map(|c| c.0), clip.map(|c| c.1));
    let color = palette.colors.first().copied();
    Ok(lib.set_meta(uid, &Meta { width, height, duration_ms, audio, color, error: None })?)
}

pub(crate) fn thumb(lib: &mut Library, item: &Item) -> Result<Option<PathBuf>, Error> {
    let path = thumb_file(&item.uid, "jpg")?;
    if !path.is_file() && item.kind == Kind::Image {
        let small = decode_image(&item.path, EDGE).map_err(|e| Error::Unreadable(e.to_string()))?;
        describe(lib, &item.uid, DynamicImage::ImageRgba8(small), image_size(&item.path).ok(), None)?;
    }
    Ok(path.is_file().then_some(path))
}

pub(crate) fn colors(lib: &mut Library, item: &Item) -> Result<Option<[i32; 4]>, Error> {
    let read = || -> Result<Option<[i32; 4]>, Error> {
        let saved = fs::read_to_string(thumb_file(&item.uid, "colors")?).unwrap_or_default();
        let parsed: Vec<_> =
            saved.split_whitespace().filter_map(|c| u32::from_str_radix(c, 16).ok()).collect();
        Ok(<[u32; 4]>::try_from(parsed).ok().map(|colors| colors.map(|c| c as i32)))
    };
    match read()? {
        Some(colors) => Ok(Some(colors)),
        None => thumb(lib, item).and_then(|_| read()),
    }
}

pub(crate) fn forget(item: &Item) -> Result<(), Error> {
    crate::depth::forget(&item.path)?;
    if item.kind == Kind::Package {
        let inner = item.package.as_ref().is_some_and(|p| p.kind == "image");
        if let Some((file, _)) = inner.then(|| pack::media(item).ok()).flatten() {
            crate::depth::forget(&file)?;
        }
        drop(fs::remove_dir_all(pack::folder(&item.path)?));
    }
    let owned = item.path.starts_with(fs::canonicalize(dir()?.join(MEDIA))?).then(|| item.path.clone());
    let animated = ["animated.gif", "animated.webp"].map(|ext| thumb_file(&item.uid, ext));
    let files = [thumb_file(&item.uid, "jpg")?, thumb_file(&item.uid, "colors")?]
        .into_iter()
        .chain(animated.into_iter().flatten());
    for path in files.chain(owned) {
        drop(fs::remove_file(path));
    }
    Ok(())
}
