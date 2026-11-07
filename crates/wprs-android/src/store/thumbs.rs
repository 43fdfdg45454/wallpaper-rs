use super::{EDGE, thumb_file, write};
use crate::{Error, library, pack};
use image::{DynamicImage, RgbaImage};
use std::fs;
use std::path::PathBuf;
use wprs_library::{Item, Kind, Library, Meta};
use wprs_render::{decode_image, image_size, palette, thumbnails};

const DARK_TEXT: i32 = 1;
const DARK_THEME: i32 = 2;

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

pub(super) fn describe(
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
