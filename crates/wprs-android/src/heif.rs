use image::RgbaImage;
use ndk_sys::{
    AImageDecoder, AImageDecoder_createFromFd, AImageDecoder_decodeImage, AImageDecoder_delete,
    AImageDecoder_getHeaderInfo, AImageDecoder_getMinimumStride, AImageDecoder_setAndroidBitmapFormat,
    AImageDecoder_setTargetSize, AImageDecoder_setUnpremultipliedRequired, AImageDecoderHeaderInfo_getHeight,
    AImageDecoderHeaderInfo_getWidth, ANDROID_IMAGE_DECODER_SUCCESS, AndroidBitmapFormat,
};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;
use wprs_render::Platform;

struct Decoder {
    raw: *mut AImageDecoder,
    _file: File,
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { AImageDecoder_delete(self.raw) };
    }
}

fn ok(code: i32) -> Option<()> {
    (code == ANDROID_IMAGE_DECODER_SUCCESS).then_some(())
}

fn open(path: &Path) -> Option<(Decoder, (u32, u32))> {
    let file = File::open(path).ok()?;
    let mut decoder = std::ptr::null_mut();
    ok(unsafe { AImageDecoder_createFromFd(file.as_raw_fd(), &mut decoder) })?;
    let decoder = Decoder { raw: decoder, _file: file };
    let info = unsafe { AImageDecoder_getHeaderInfo(decoder.raw) };
    let size = unsafe { (AImageDecoderHeaderInfo_getWidth(info), AImageDecoderHeaderInfo_getHeight(info)) };
    Some((decoder, (size.0.max(1).unsigned_abs(), size.1.max(1).unsigned_abs())))
}

fn decode(path: &Path, max_edge: u32) -> Option<RgbaImage> {
    let (decoder, (w, h)) = open(path)?;
    let scale = (f64::from(max_edge.max(1)) / f64::from(w.max(h))).min(1.0);
    let (w, h) =
        ((f64::from(w) * scale).round().max(1.0) as u32, (f64::from(h) * scale).round().max(1.0) as u32);
    let rgba = AndroidBitmapFormat::ANDROID_BITMAP_FORMAT_RGBA_8888.0 as i32;
    ok(unsafe { AImageDecoder_setAndroidBitmapFormat(decoder.raw, rgba) })?;
    unsafe { AImageDecoder_setUnpremultipliedRequired(decoder.raw, true) };
    ok(unsafe { AImageDecoder_setTargetSize(decoder.raw, w as i32, h as i32) })?;
    let stride = unsafe { AImageDecoder_getMinimumStride(decoder.raw) };
    let mut pixels = vec![0u8; stride * h as usize];
    ok(unsafe { AImageDecoder_decodeImage(decoder.raw, pixels.as_mut_ptr().cast(), stride, pixels.len()) })?;
    let row = w as usize * 4;
    let packed = pixels.chunks_exact(stride).flat_map(|line| &line[..row]).copied().collect();
    RgbaImage::from_raw(w, h, packed)
}

pub(crate) fn register() {
    wprs_library::platform_decodes_heif();
    wprs_render::set_platform_decoder(Platform { decode, size: |path| open(path).map(|(_, size)| size) });
}
