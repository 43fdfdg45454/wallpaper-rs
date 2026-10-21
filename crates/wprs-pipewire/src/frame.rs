use pipewire::spa::buffer::Data;

pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub data: Vec<u8>,
}

pub fn write_into(datas: &mut [Data], frame: Option<&VideoFrame>, width: u32, height: u32) {
    let Some(data) = datas.first_mut() else { return };
    let stride = width * 4;
    let written = data.data().map_or(0, |dst| match frame {
        Some(frame) => copy_frame(frame, dst, width, height, stride),
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

fn copy_frame(frame: &VideoFrame, dst: &mut [u8], width: u32, height: u32, stride: u32) -> usize {
    let row_bytes = (width.min(frame.width) * 4) as usize;
    let total = (stride * height) as usize;
    if dst.len() < total {
        return 0;
    }
    if (frame.width, frame.height) != (width, height) {
        dst[..total].fill(0);
    }
    for y in 0..height.min(frame.height) as usize {
        let src = &frame.data[y * frame.stride as usize..][..row_bytes];
        dst[y * stride as usize..][..row_bytes].copy_from_slice(src);
    }
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
        assert_eq!(copy_frame(&frame(4, 3, 7), &mut dst, 4, 3, 16), 48);
        assert!(dst.iter().all(|&b| b == 7));
    }

    #[test]
    fn crops_larger_frames_and_pads_smaller_ones() {
        let mut dst = vec![1u8; 4 * 2 * 4];
        assert_eq!(copy_frame(&frame(8, 8, 9), &mut dst, 4, 2, 16), 32);
        assert!(dst.iter().all(|&b| b == 9));

        let mut dst = vec![1u8; 4 * 2 * 4];
        copy_frame(&frame(2, 1, 5), &mut dst, 4, 2, 16);
        assert_eq!(&dst[..8], &[5; 8]);
        assert!(dst[8..].iter().all(|&b| b == 0));
    }

    #[test]
    fn refuses_short_destination() {
        assert_eq!(copy_frame(&frame(4, 4, 1), &mut [0u8; 10], 4, 4, 16), 0);
    }

    #[test]
    fn fills_whole_pixels_with_opaque_black() {
        let mut dst = [1u8; 11];
        assert_eq!(fill_black(&mut dst, 32), 11);
        assert_eq!(dst, [0, 0, 0, 255, 0, 0, 0, 255, 1, 1, 1]);
    }
}
