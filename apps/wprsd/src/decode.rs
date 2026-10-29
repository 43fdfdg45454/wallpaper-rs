use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbaImage};
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Seek, Write};
use std::path::Path;
use wprs_format::{Kind as PackageKind, Package, Platform, features};
use wprs_library::Meta;

pub struct Picture {
    pub meta: Meta,
    pub image: Option<DynamicImage>,
    pub failure: Option<String>,
}

pub enum Problem {
    Missing(String),
    Decode(String),
}

impl<E: std::error::Error + 'static> From<E> for Problem {
    fn from(e: E) -> Self {
        let text = e.to_string();
        let source = std::iter::successors(Some(&e as &(dyn std::error::Error + 'static)), |e| e.source());
        let io = source.filter_map(|e| e.downcast_ref::<std::io::Error>()).next().map(std::io::Error::kind);
        match io {
            Some(std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied) => {
                Problem::Missing(text)
            }
            _ => Problem::Decode(text),
        }
    }
}

impl Picture {
    pub fn of(image: DynamicImage) -> Picture {
        let meta = Meta { width: Some(image.width()), height: Some(image.height()), ..Meta::default() };
        Picture { meta, image: Some(image), failure: None }
    }
}

pub fn decode<R: BufRead + Seek>(reader: ImageReader<R>) -> Result<DynamicImage, Problem> {
    let mut reader = reader.with_guessed_format()?;
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height, limits.max_alloc) =
        (Some(1 << 15), Some(1 << 15), Some(1 << 30));
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder)?;
    image.apply_orientation(orientation);
    Ok(image)
}

fn bytes(data: Vec<u8>) -> Result<DynamicImage, Problem> {
    decode(ImageReader::new(Cursor::new(data)))
}

pub fn video(path: &Path) -> Result<Picture, Problem> {
    let still = wprs_video::still(path).map_err(|e| match e {
        wprs_video::Error::NotLocal(_) => Problem::Missing(e.to_string()),
        e => Problem::Decode(e.to_string()),
    })?;
    let (duration_ms, audio) = (still.duration_ms, Some(still.audio));
    let frame = RgbaImage::from_raw(still.width, still.height, still.rgba).map(DynamicImage::ImageRgba8);
    let picture = Picture::of(frame.ok_or_else(|| Problem::Decode("the video frame is incomplete".into()))?);
    Ok(Picture { meta: Meta { duration_ms, audio, ..picture.meta }, ..picture })
}

pub fn open(path: &Path) -> wprs_format::Result<Package<BufReader<File>>> {
    let package = wprs_format::open(path, &wprs_format::Limits::default())?;
    features::playable(&package.manifest, &package.facts, Platform::Desktop)?;
    Ok(package)
}

pub fn package(path: &Path) -> Result<Picture, Problem> {
    let mut package: Package<_> = match open(path) {
        Err(e @ (wprs_format::Error::Newer(_) | wprs_format::Error::Elsewhere(_))) => {
            return Ok(newer(path, e.to_string()));
        }
        package => package?,
    };
    let manifest = package.manifest.clone();
    let preview = manifest.thumbnail().map(|p| bytes(package.read_file(p)?)).transpose()?;
    let entry = package.read_file(&manifest.entry);
    let entry = match manifest.kind {
        PackageKind::Scene => {
            let [width, height] = package.scene()?.canvas.size.map(Some);
            let failure = preview.is_none().then(|| "no preview".to_owned());
            return Ok(Picture { meta: Meta { width, height, ..Meta::default() }, image: preview, failure });
        }
        PackageKind::Web | PackageKind::Preset | PackageKind::Assets => {
            return Ok(Picture { meta: Meta::default(), image: preview, failure: None });
        }
        PackageKind::Image => Picture::of(bytes(entry?)?),
        PackageKind::Video => {
            let extension = Path::new(&manifest.entry).extension().unwrap_or_default().to_string_lossy();
            let mut file = tempfile::Builder::new().suffix(&format!(".{extension}")).tempfile()?;
            file.write_all(&entry?)?;
            video(file.path())?
        }
    };
    Ok(Picture { image: preview.or(entry.image), ..entry })
}

fn newer(path: &Path, error: String) -> Picture {
    let image = wprs_format::thumbnail(path).ok().flatten().and_then(|b| bytes(b).ok());
    Picture { meta: Meta { error: Some(error), ..Meta::default() }, image, failure: None }
}
