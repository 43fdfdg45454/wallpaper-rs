use crate::{Error, Info, Levels, Target, sized};
use basisu::{DecodeFlags, Transcoder, VideoState};
use std::f32::consts::{FRAC_PI_2, PI};

pub fn grid(frames: u32) -> (u32, u32) {
    let columns = (frames.max(1) as f32).sqrt().ceil().max(1.0) as u32;
    (frames.max(1).div_ceil(columns), columns)
}

pub(crate) fn sheet(t: &Transcoder<'_>, info: &Info, level: u32, target: Target) -> Result<Levels, Error> {
    let (w, h) = sized(info, level);
    let (side, unit) = target.unit();
    if w % side != 0 || h % side != 0 {
        return Err(Error::Unsupported);
    }
    let ((rows, columns), (across, down)) = (grid(info.frames), (w / side, h / side));
    let row = across as usize * unit;
    let mut data = vec![0u8; row * columns as usize * down as usize * rows as usize];
    let mut state = VideoState::new();
    for i in 0..info.frames {
        let frame = t
            .transcode_video_frame(&mut state, level, i, target.basis(), DecodeFlags::HIGH_QUALITY)
            .map_err(|_| Error::Broken)?;
        let (x, y) = ((i % columns) as usize, (i / columns) as usize);
        for r in 0..down as usize {
            let at = ((y * down as usize + r) * columns as usize + x) * row;
            data[at..at + row].copy_from_slice(frame.get(r * row..(r + 1) * row).ok_or(Error::Broken)?);
        }
    }
    let (width, height) = (w * columns, h * rows);
    Ok(Levels { target, width, height, padded: (width, height), count: 1, data })
}

fn face(d: [f32; 3]) -> (u32, f32, f32) {
    let [x, y, z] = d;
    let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
    let (index, sc, tc, ma) = match () {
        _ if ax >= ay && ax >= az && x > 0.0 => (0, -z, -y, ax),
        _ if ax >= ay && ax >= az => (1, z, -y, ax),
        _ if ay >= az && y > 0.0 => (2, x, z, ay),
        _ if ay >= az => (3, x, -z, ay),
        _ if z > 0.0 => (4, x, -y, az),
        _ => (5, -x, -y, az),
    };
    (index, (sc / ma + 1.0) / 2.0, (tc / ma + 1.0) / 2.0)
}

pub(crate) fn panorama(t: &Transcoder<'_>, info: &Info, level: u32, target: Target) -> Result<Levels, Error> {
    if target.compressed() {
        return Err(Error::Unsupported);
    }
    let side = sized(info, level).0;
    let unit = target.unit().1;
    let decode = |f: u32| t.transcode_image(level, 0, f, target.basis(), DecodeFlags::HIGH_QUALITY);
    let faces: Vec<Vec<u8>> = (0..6).map(decode).collect::<Result<_, _>>().map_err(|_| Error::Broken)?;
    let (width, height) = (side * 4, side * 2);
    let mut data = Vec::with_capacity(width as usize * height as usize * unit);
    for v in 0..height {
        let lat = FRAC_PI_2 - (v as f32 + 0.5) / height as f32 * PI;
        for u in 0..width {
            let lon = (u as f32 + 0.5) / width as f32 * 2.0 * PI - PI;
            let (f, s, tt) = face([lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos()]);
            let texel = |c: f32| ((c * side as f32) as u32).min(side - 1) as usize;
            let at = (texel(tt) * side as usize + texel(s)) * unit;
            data.extend_from_slice(faces[f as usize].get(at..at + unit).ok_or(Error::Broken)?);
        }
    }
    Ok(Levels { target, width, height, padded: (width, height), count: 1, data })
}

pub(crate) fn clamped(half: &[u8]) -> Vec<u8> {
    use crate::light::{alpha, encoded};
    let channel = |b: &[u8; 8], i: usize| half::f16::from_le_bytes([b[2 * i], b[2 * i + 1]]).to_f32();
    let texel = |b: &[u8; 8]| {
        [encoded(channel(b, 0)), encoded(channel(b, 1)), encoded(channel(b, 2)), alpha(channel(b, 3))]
    };
    half.as_chunks::<8>().0.iter().flat_map(texel).collect()
}
