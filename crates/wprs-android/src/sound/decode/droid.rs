use super::Chunk;
use ndk::media::media_codec::MediaCodecDirection;
use ndk::media::media_codec::{DequeuedInputBufferResult, DequeuedOutputBufferInfoResult, MediaCodec};
use ndk::media::media_format::MediaFormat;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

const END: u32 = 4;
const FLOATS: i32 = 4;
const TURN: Duration = Duration::from_millis(5);
const TRIES: u32 = 400;

pub(crate) struct Decoder {
    extractor: NonNull<ndk_sys::AMediaExtractor>,
    codec: MediaCodec,
    _file: File,
    fed: bool,
    rate: u32,
    channels: usize,
    float: bool,
}

unsafe impl Send for Decoder {}

fn track(extractor: NonNull<ndk_sys::AMediaExtractor>) -> Option<(MediaFormat, String)> {
    (0..unsafe { ndk_sys::AMediaExtractor_getTrackCount(extractor.as_ptr()) }).find_map(|i| {
        let found = unsafe { ndk_sys::AMediaExtractor_getTrackFormat(extractor.as_ptr(), i) };
        let mut format = unsafe { MediaFormat::from_ptr(NonNull::new(found)?) };
        let mime = format.str("mime")?.to_owned();
        mime.starts_with("audio/").then(|| unsafe {
            ndk_sys::AMediaExtractor_selectTrack(extractor.as_ptr(), i);
            (format, mime)
        })
    })
}

impl Decoder {
    pub(crate) fn open(path: &Path) -> Result<Decoder, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let size = file.metadata().map_err(|e| e.to_string())?.len();
        let extractor = NonNull::new(unsafe { ndk_sys::AMediaExtractor_new() }).ok_or("no extractor")?;
        let mut decoder = Decoder::wrap(extractor, file, size)?;
        decoder.start()?;
        Ok(decoder)
    }

    fn wrap(extractor: NonNull<ndk_sys::AMediaExtractor>, file: File, size: u64) -> Result<Decoder, String> {
        let status = unsafe {
            ndk_sys::AMediaExtractor_setDataSourceFd(extractor.as_ptr(), file.as_raw_fd(), 0, size as i64)
        };
        let found = if status.0 == 0 { track(extractor) } else { None };
        let (format, mime) = found.ok_or("has no sound track")?;
        let codec = MediaCodec::from_decoder_type(&mime).ok_or("no decoder")?;
        codec.configure(&format, None, MediaCodecDirection::Decoder).map_err(|e| e.to_string())?;
        let (rate, channels) =
            (format.i32("sample-rate").unwrap_or(48_000), format.i32("channel-count").unwrap_or(2));
        let (rate, channels) = (rate.max(1) as u32, channels.max(1) as usize);
        Ok(Decoder { extractor, codec, _file: file, fed: false, rate, channels, float: false })
    }

    fn start(&mut self) -> Result<(), String> {
        self.codec.start().map_err(|e| e.to_string())
    }

    pub(crate) fn seek(&mut self, us: i64) {
        unsafe {
            ndk_sys::AMediaExtractor_seekTo(
                self.extractor.as_ptr(),
                us,
                ndk_sys::SeekMode::AMEDIAEXTRACTOR_SEEK_PREVIOUS_SYNC,
            )
        };
        let _ = self.codec.flush();
        self.fed = false;
    }

    fn feed(&mut self) {
        let Ok(DequeuedInputBufferResult::Buffer(mut input)) =
            self.codec.dequeue_input_buffer(Duration::ZERO)
        else {
            return;
        };
        let buffer = input.buffer_mut();
        let read = unsafe {
            ndk_sys::AMediaExtractor_readSampleData(
                self.extractor.as_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        let time = unsafe { ndk_sys::AMediaExtractor_getSampleTime(self.extractor.as_ptr()) }.max(0) as u64;
        let queued = match usize::try_from(read) {
            Ok(size) => self.codec.queue_input_buffer(input, 0, size, time, 0),
            Err(_) => {
                self.fed = true;
                self.codec.queue_input_buffer(input, 0, 0, 0, END)
            }
        };
        if queued.is_ok() && !self.fed {
            unsafe { ndk_sys::AMediaExtractor_advance(self.extractor.as_ptr()) };
        }
    }

    pub(crate) fn next(&mut self) -> Option<Chunk> {
        for _ in 0..TRIES {
            if !self.fed {
                self.feed();
            }
            match self.codec.dequeue_output_buffer(TURN).ok()? {
                DequeuedOutputBufferInfoResult::Buffer(out) => {
                    let info = *out.info();
                    let (from, len) = (info.offset().max(0) as usize, info.size().max(0) as usize);
                    let bytes = out.buffer().get(from..from + len).unwrap_or_default();
                    let samples: Vec<i16> = if self.float {
                        let floats = bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b));
                        floats.map(|f| (f.clamp(-1.0, 1.0) * 32767.0) as i16).collect()
                    } else {
                        bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect()
                    };
                    let last = info.flags() & END != 0;
                    let _ = self.codec.release_output_buffer(out, false);
                    if !samples.is_empty() {
                        let (rate, channels, start_us) =
                            (self.rate, self.channels, info.presentation_time_us());
                        return Some(Chunk { rate, channels, start_us, samples });
                    }
                    if last {
                        return None;
                    }
                }
                DequeuedOutputBufferInfoResult::OutputFormatChanged => {
                    let format = self.codec.output_format();
                    self.rate = format.i32("sample-rate").map_or(self.rate, |r| r.max(1) as u32);
                    self.channels = format.i32("channel-count").map_or(self.channels, |c| c.max(1) as usize);
                    self.float = format.i32("pcm-encoding") == Some(FLOATS);
                }
                _ => {}
            }
        }
        None
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.codec.stop();
        unsafe { ndk_sys::AMediaExtractor_delete(self.extractor.as_ptr()) };
    }
}
