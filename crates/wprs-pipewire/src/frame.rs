use wprs_core::Region;

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

    pub fn patch(&mut self, src: &VideoFrame<&[u8]>, damage: Region) {
        let (from, to, stride) =
            ((damage.x * 4) as usize, ((damage.x + damage.width) * 4) as usize, self.stride);
        let rows = self.data.chunks_exact_mut(stride as usize).zip(src.rows()).skip(damage.y as usize);
        rows.take(damage.height as usize).for_each(|(d, s)| d[from..to].copy_from_slice(&s[from..to]));
    }

    pub fn assign(&mut self, src: &VideoFrame<&[u8]>) {
        self.data.clear();
        self.data.reserve(src.width as usize * 4 * src.height as usize);
        src.rows().for_each(|row| self.data.extend_from_slice(row));
        (self.width, self.height, self.stride) = (src.width, src.height, src.width * 4);
    }
}

pub(crate) fn copy_frame(
    frame: &VideoFrame<impl AsRef<[u8]>>,
    dst: &mut [u8],
    (width, height): (u32, u32),
    since: Option<Region>,
) -> usize {
    let (stride, total) = (width * 4, (width * 4 * height) as usize);
    let Some(dst) = dst.get_mut(..total).filter(|_| stride > 0) else { return 0 };
    let same = (frame.width, frame.height) == (width, height);
    if !same {
        dst.fill(0);
    }
    let whole = Region::whole((width.min(frame.width), height.min(frame.height)));
    let r = since.filter(|_| same).map_or(whole, |r| r.within((width, height)));
    let (from, to) = ((r.x * 4) as usize, ((r.x + r.width) * 4) as usize);
    let rows = dst.chunks_exact_mut(stride as usize).zip(frame.rows()).skip(r.y as usize);
    rows.take(r.height as usize).for_each(|(d, s)| d[from..to].copy_from_slice(&s[from..to]));
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
        assert_eq!(copy_frame(&frame(4, 3, 7), &mut dst, (4, 3), None), 48);
        assert!(dst.iter().all(|&b| b == 7));
    }

    #[test]
    fn crops_larger_frames_and_pads_smaller_ones() {
        let mut dst = vec![1u8; 4 * 2 * 4];
        assert_eq!(copy_frame(&frame(8, 8, 9), &mut dst, (4, 2), None), 32);
        assert!(dst.iter().all(|&b| b == 9));

        let mut dst = vec![1u8; 4 * 2 * 4];
        copy_frame(&frame(2, 1, 5), &mut dst, (4, 2), None);
        assert_eq!(&dst[..8], &[5; 8]);
        assert!(dst[8..].iter().all(|&b| b == 0));
    }

    #[test]
    fn copies_only_the_damaged_rectangle_of_a_frame_of_the_same_size() {
        let mut dst = vec![1u8; 4 * 3 * 4];
        let damage = Region { x: 1, y: 1, width: 2, height: 1 };
        assert_eq!(copy_frame(&frame(4, 3, 7), &mut dst, (4, 3), Some(damage)), 48);
        let changed: Vec<usize> = (0..12).filter(|p| dst[p * 4] == 7).collect();
        assert_eq!(changed, [5, 6]);
        copy_frame(&frame(2, 3, 9), &mut dst, (4, 3), Some(damage));
        assert!(dst.chunks(16).all(|row| row[..8] == [9; 8] && row[8..] == [0; 8]), "{dst:?}");
    }

    #[test]
    fn refuses_short_destination() {
        assert_eq!(copy_frame(&frame(4, 4, 1), &mut [0u8; 10], (4, 4), None), 0);
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
        assert_eq!(copy_frame(&src, &mut dst, (2, 3), None), 24);
        assert_eq!(dst, own.data);
    }
}
