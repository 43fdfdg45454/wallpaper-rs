use pipewire::spa::buffer::Data;

#[derive(Default)]
pub struct VideoFrame<D = Vec<u8>> {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: D,
}

impl<D: AsRef<[u8]>> VideoFrame<D> {
    pub fn is_valid(&self) -> bool {
        let (row, stride) = (u64::from(self.width) * 4, u64::from(self.stride));
        let len = self.data.as_ref().len() as u64;
        row > 0 && stride >= row && self.height > 0 && stride * u64::from(self.height - 1) + row <= len
    }

    fn rows(&self) -> impl Iterator<Item = &[u8]> {
        let row = self.width as usize * 4;
        self.data
            .as_ref()
            .chunks(self.stride.max(1) as usize)
            .take(self.height as usize)
            .map(move |r| &r[..row])
    }
}

impl VideoFrame {
    pub fn copy_of(src: &VideoFrame<&[u8]>) -> Self {
        let mut frame = Self::default();
        frame.assign(src);
        frame
    }

    pub fn assign(&mut self, src: &VideoFrame<&[u8]>) {
        self.data.clear();
        self.data.reserve(src.width as usize * 4 * src.height as usize);
        src.rows().for_each(|row| self.data.extend_from_slice(row));
        (self.width, self.height, self.stride) = (src.width, src.height, src.width * 4);
    }
}

pub fn write_into(datas: &mut [Data], frame: Option<&VideoFrame>, width: u32, height: u32) {
    let Some(data) = datas.first_mut() else { return };
    let stride = width * 4;
    let written = data.data().map_or(0, |dst| match frame {
        Some(frame) => copy_frame(frame, dst, width, height),
        None => fill_black(dst, (stride * height) as usize),
    });
    tracing::trace!(written, data_type = ?data.type_(), "process: frame queued");
    let chunk = data.chunk_mut();
    *chunk.offset_mut() = 0;
    *chunk.stride_mut() = stride as i32;
    *chunk.size_mut() = written as u32;
}

fn fill_black(dst: &mut [u8], len: usize) -> usize {
    let len = dst.len().min(len);
    dst[..len].as_chunks_mut::<4>().0.fill([0, 0, 0, 255]);
    len
}

fn copy_frame(frame: &VideoFrame<impl AsRef<[u8]>>, dst: &mut [u8], width: u32, height: u32) -> usize {
    let (stride, total) = (width * 4, (width * 4 * height) as usize);
    let Some(dst) = dst.get_mut(..total).filter(|_| stride > 0) else { return 0 };
    if (frame.width, frame.height) != (width, height) {
        dst.fill(0);
    }
    let row = (width.min(frame.width) * 4) as usize;
    dst.chunks_exact_mut(stride as usize)
        .zip(frame.rows())
        .for_each(|(d, s)| d[..row].copy_from_slice(&s[..row]));
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32, v: u8) -> VideoFrame {
        VideoFrame { width: w, height: h, stride: w * 4, data: vec![v; (w * h * 4) as usize] }
    }

    #[test]
    fn copies_matching_frames() {
        let mut dst = vec![0u8; 4 * 3 * 4];
        assert_eq!(copy_frame(&frame(4, 3, 7), &mut dst, 4, 3), 48);
        assert!(dst.iter().all(|&b| b == 7));
    }

    #[test]
    fn crops_larger_frames_and_pads_smaller_ones() {
        let mut dst = vec![1u8; 4 * 2 * 4];
        assert_eq!(copy_frame(&frame(8, 8, 9), &mut dst, 4, 2), 32);
        assert!(dst.iter().all(|&b| b == 9));

        let mut dst = vec![1u8; 4 * 2 * 4];
        copy_frame(&frame(2, 1, 5), &mut dst, 4, 2);
        assert_eq!(&dst[..8], &[5; 8]);
        assert!(dst[8..].iter().all(|&b| b == 0));
    }

    #[test]
    fn refuses_short_destination() {
        assert_eq!(copy_frame(&frame(4, 4, 1), &mut [0u8; 10], 4, 4), 0);
    }

    #[test]
    fn fills_whole_pixels_with_opaque_black() {
        let mut dst = [1u8; 11];
        assert_eq!(fill_black(&mut dst, 32), 11);
        assert_eq!(dst, [0, 0, 0, 255, 0, 0, 0, 255, 1, 1, 1]);
    }

    #[test]
    fn validates_geometry() {
        let padded = [0u8; 2 * 12 + 8];
        assert!(VideoFrame { width: 2, height: 3, stride: 12, data: &padded[..] }.is_valid());
        assert!(!VideoFrame { width: 2, height: 3, stride: 12, data: &padded[1..] }.is_valid());
        assert!(!VideoFrame { width: 4, height: 1, stride: 12, data: &padded[..] }.is_valid());
        assert!(!VideoFrame { width: 0, height: 1, stride: 0, data: &padded[..] }.is_valid());
        assert!(!VideoFrame { width: 1, height: 0, stride: 4, data: &padded[..] }.is_valid());
    }

    #[test]
    fn assigns_padded_rows_without_reallocating() {
        let padded: Vec<u8> = (0..2 * 12 + 8).collect();
        let src = VideoFrame { width: 2, height: 3, stride: 12, data: &padded[..] };
        let mut own = VideoFrame::copy_of(&src);
        assert_eq!((own.stride, own.data.len()), (8, 24));
        assert_eq!(&own.data[8..16], &padded[12..20]);
        let ptr = own.data.as_ptr();
        own.assign(&src);
        assert_eq!(own.data.as_ptr(), ptr);
        let mut dst = vec![0u8; 24];
        assert_eq!(copy_frame(&src, &mut dst, 2, 3), 24);
        assert_eq!(dst, own.data);
    }
}
