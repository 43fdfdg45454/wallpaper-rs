use crate::validate::extension;
use crate::{Package, Result, media};
use std::io::{self, Read, Seek};

pub const PHONE_BYTES: u64 = 1 << 30;
pub const PHONE_SIDE: u32 = 8192;
pub const PHONE_PIXELS: u64 = 32 << 20;
const PICTURES: [&str; 5] = ["png", "jpg", "jpeg", "webp", "gif"];
pub(crate) const HEAD: u64 = 1 << 16;

pub(crate) fn picture(path: &str) -> bool {
    extension(path).is_some_and(|x| PICTURES.contains(&x.as_str()))
}

pub(crate) fn oversized(total: u64, heads: Vec<(Vec<u8>, String)>) -> Option<String> {
    if total > PHONE_BYTES {
        let mib = total >> 20;
        return Some(format!("it takes {mib} MiB once unpacked, more than the 1 GiB a phone reads"));
    }
    heads.into_iter().find_map(|(head, path)| {
        let m = media::probe(&head)?;
        let big =
            m.width.max(m.height) > PHONE_SIDE || u64::from(m.width) * u64::from(m.height) > PHONE_PIXELS;
        let (w, h) = (m.width, m.height);
        big.then(|| format!("{path} is {w}×{h} pixels, more than the 8192 a side and 32 Mpx a phone draws"))
    })
}

impl<R: Read + Seek> Package<R> {
    pub fn look_closer(&mut self) -> Result<()> {
        for path in self.videos.clone() {
            let (head, tail) = self.ends(&path, media::EDGE)?;
            if media::hdr(&head) || tail.is_some_and(|t| media::hdr(&t)) {
                media::mark(&mut self.facts, &path);
            }
        }
        if let Some(note) = self.too_large_for_phones()? {
            self.facts.facets.retain(|f| f != "android");
            self.facts.notes.push(note);
        }
        Ok(())
    }

    fn too_large_for_phones(&mut self) -> Result<Option<String>> {
        let total: u64 = self.entries.iter().map(|e| e.size).sum();
        let paths: Vec<String> = self.entries.iter().map(|e| e.path.clone()).filter(|p| picture(p)).collect();
        let mut heads = Vec::with_capacity(paths.len());
        for path in paths {
            heads.push((self.head(&path, HEAD)?, path));
        }
        Ok(oversized(total, heads))
    }

    pub(crate) fn ends(&mut self, path: &str, edge: u64) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
        let head = self.head(path, edge)?;
        let Ok(mut file) = self.archive.by_name_seek(path) else { return Ok((head, None)) };
        let size = file.seek(io::SeekFrom::End(0))?;
        file.seek(io::SeekFrom::Start(size.saturating_sub(edge).max(edge)))?;
        let mut tail = Vec::new();
        file.read_to_end(&mut tail)?;
        Ok((head, Some(tail)))
    }
}
