use super::helper::{Page, available, with};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use wprs_web::{Isolation, Settings, ToWeb};

fn tone() -> Vec<u8> {
    let samples: Vec<i16> = (0..8000).map(|i| ((i as f32 * 0.25).sin() * 8000.0) as i16).collect();
    let data = samples.len() as u32 * 2;
    let mut wav = Vec::new();
    wav.extend(b"RIFF");
    wav.extend((36 + data).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend([1u16, 1].iter().flat_map(|v| v.to_le_bytes()));
    wav.extend([8000u32, 16000].iter().flat_map(|v| v.to_le_bytes()));
    wav.extend([2u16, 16].iter().flat_map(|v| v.to_le_bytes()));
    wav.extend(b"data");
    wav.extend(data.to_le_bytes());
    wav.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
    wav
}

fn until(page: &mut Page, name: &str, want: impl Fn(&Value) -> bool) -> Value {
    let end = Instant::now() + Duration::from_secs(20);
    while Instant::now() < end && !page.said.get(name).and_then(|v| v.last()).is_some_and(&want) {
        page.pump(Instant::now() + Duration::from_millis(20));
    }
    page.said.get(name).and_then(|v| v.last()).cloned().unwrap_or(Value::Null)
}

#[test]
fn packaged_sound_and_video_play_at_the_wallpaper_volume() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let (wav, clip) = (tone(), include_bytes!("../../../../tests/fixtures/magenta-64x32.webm"));
    let extra: [(&str, &[u8]); 3] =
        [("probe.js", include_bytes!("media.js")), ("tone.wav", &wav), ("clip.webm", clip)];
    let mut start = with(dir.path(), 9, &extra);
    start.settings = Settings { volume: 0.5, ..start.settings };
    let mut page = Page::open(start, Isolation { network: true, devices: false, limits: false });
    assert_eq!(page.wait("audio", 20), json!(["played", false, 1]));
    assert_eq!(page.wait("video", 20), json!(["played", false, 1]));
    assert_eq!(page.wait("context", 10), json!(["running", 0.5]));
    let settings = Settings { volume: 0.25, ..Settings::default() };
    page.link.send(&ToWeb::Settings(settings), &[]).unwrap();
    let want = json!([0.25, 0.25]);
    assert_eq!(until(&mut page, "volume", |v| v == &want), want);
}
