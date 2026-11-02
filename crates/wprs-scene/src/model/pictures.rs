use super::{MAX_MODEL_TEXELS, MAX_TEXTURE, ModelTexture};
use crate::rigged::read::Bin;

fn word(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|w| u32::from_be_bytes([w[0], w[1], w[2], w[3]]))
}

fn jpeg(b: &[u8]) -> Option<[u32; 2]> {
    let mut at = 2;
    while at + 9 < b.len() {
        if b[at] != 0xFF {
            return None;
        }
        let (marker, len) = (b[at + 1], usize::from(u16::from_be_bytes([b[at + 2], b[at + 3]])));
        if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let h = u32::from(u16::from_be_bytes([b[at + 5], b[at + 6]]));
            let w = u32::from(u16::from_be_bytes([b[at + 7], b[at + 8]]));
            return Some([w, h]);
        }
        at += 2 + len;
    }
    None
}

pub fn picture_size(b: &[u8]) -> Option<[u32; 2]> {
    match b {
        [0x89, b'P', b'N', b'G', ..] => Some([word(b, 16)?, word(b, 20)?]),
        [0xFF, 0xD8, ..] => jpeg(b),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => webp(b),
        _ => None,
    }
}

fn webp(b: &[u8]) -> Option<[u32; 2]> {
    let le = |at: usize| Some(u32::from(*b.get(at)?) | u32::from(*b.get(at + 1)?) << 8);
    match b.get(12..16)? {
        b"VP8 " => Some([le(26)? & 0x3FFF, le(28)? & 0x3FFF]),
        b"VP8L" => {
            let bits = u32::from_le_bytes([*b.get(21)?, *b.get(22)?, *b.get(23)?, *b.get(24)?]);
            Some([(bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1])
        }
        b"VP8X" => {
            let three = |at: usize| Some(u32::from(*b.get(at)?) | le(at + 1)? << 8);
            Some([three(24)? + 1, three(27)? + 1])
        }
        _ => None,
    }
}

pub(super) fn textures(bin: &Bin) -> Result<(Vec<ModelTexture>, Vec<u16>), String> {
    let doc = bin.doc;
    if doc.textures.len() > crate::MAX_MODEL_PARTS {
        let most = crate::MAX_MODEL_PARTS;
        return Err(format!("has {} textures, at most {most} are allowed", doc.textures.len()));
    }
    let (mut out, mut held, mut map, mut texels, mut copied) = (Vec::new(), Vec::new(), Vec::new(), 0u64, 0);
    for (t, texture) in doc.textures.iter().enumerate() {
        let image = texture.source.and_then(|s| doc.images.get(s));
        let view =
            image.and_then(|i| i.view).ok_or_else(|| format!("texture {t} has no picture in the file"))?;
        let span = bin.view(view)?;
        let range = (doc.views[view].offset, span.len());
        if let Some(at) = held.iter().position(|r| *r == range) {
            map.push(at as u16);
            continue;
        }
        copied += span.len();
        if copied > bin.bin.len() {
            return Err(crate::gltf::PICTURES.into());
        }
        let bytes = span.to_vec();
        let size =
            picture_size(&bytes).ok_or_else(|| format!("texture {t} is not a PNG, JPEG or WebP picture"))?;
        if size[0].max(size[1]) > MAX_TEXTURE {
            return Err(format!(
                "has a texture of {} px, at most {MAX_TEXTURE} are allowed",
                size[0].max(size[1])
            ));
        }
        if size.contains(&0) {
            return Err(format!("texture {t} has no pixels"));
        }
        texels += u64::from(size[0]) * u64::from(size[1]);
        if texels > MAX_MODEL_TEXELS {
            return Err(format!("has more than {MAX_MODEL_TEXELS} texture pixels"));
        }
        map.push(out.len() as u16);
        held.push(range);
        out.push(ModelTexture { bytes, size });
    }
    Ok((out, map))
}
