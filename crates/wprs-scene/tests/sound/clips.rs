use super::*;

fn wav(rate: u32, channels: u16, bytes: u32) -> Vec<u8> {
    let per_second = rate * u32::from(channels);
    let mut out =
        [&b"RIFF"[..], &(36 + bytes).to_le_bytes(), b"WAVEfmt ", &16u32.to_le_bytes(), &1u16.to_le_bytes()]
            .concat();
    out.extend(
        [
            &channels.to_le_bytes()[..],
            &rate.to_le_bytes(),
            &per_second.to_le_bytes(),
            &channels.to_le_bytes(),
            &8u16.to_le_bytes(),
        ]
        .concat(),
    );
    out.extend([&b"data"[..], &bytes.to_le_bytes()].concat());
    out.resize(out.len() + bytes as usize, 128);
    out
}

fn page(flags: u8, granule: u64, packet: &[u8]) -> Vec<u8> {
    let fixed = [&b"OggS\0"[..], &[flags], &granule.to_le_bytes(), &7u32.to_le_bytes(), &[0; 8]].concat();
    let lacing = if packet.is_empty() { vec![0] } else { vec![1, packet.len() as u8] };
    let mut page = [fixed, lacing, packet.to_vec()].concat();
    let crc = page.iter().fold(0u32, |crc, &b| {
        (0..8).fold(crc ^ u32::from(b) << 24, |c, _| if c >> 31 == 1 { c << 1 ^ 0x04c1_1db7 } else { c << 1 })
    });
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    page
}

fn ogg(head: &[u8], last: u64) -> Vec<u8> {
    [page(2, 0, head), page(4, last, b"x")].concat()
}

#[test]
fn clips_report_their_length_and_long_ones_are_refused() {
    let seconds =
        |bytes: &[u8]| Clip::probe("a.ogg", bytes).map(|c| (c.channels, (c.seconds * 100.0).round() / 100.0));
    assert_eq!(seconds(&wav(8000, 1, 80_000)), Ok((1, 10.0)));
    assert!(seconds(&wav(8000, 1, 488_000)).unwrap_err().contains("at most 60 s"));
    assert!(seconds(&wav(8000, 6, 4800)).unwrap_err().contains("6 channels"));
    let info = (44_100u64 << 44) | (1 << 41) | (15 << 36) | 441_000;
    let flac = [&b"fLaC\0\0\0\x22"[..], &[0; 10], &info.to_be_bytes(), &[0; 16]].concat();
    assert_eq!(seconds(&flac), Ok((2, 10.0)));
    let vorbis = [&b"\x01vorbis\0\0\0\0\x01"[..], &22_050u32.to_le_bytes(), &[0; 14]].concat();
    assert_eq!(seconds(&ogg(&vorbis, 22_050 * 5)), Ok((1, 5.0)));
    let mut corrupt = page(4, 22_050, b"x");
    corrupt[22] ^= 1;
    let after = [page(0, 22_050, b"x"), corrupt, page(0, 22_050, b"")];
    let tails = after.map(|tail| seconds(&[ogg(&vorbis, 22_050 * 5), tail].concat()));
    assert!(tails.iter().all(|t| *t == Ok((1, 5.0))), "a later page or a wrong checksum counted: {tails:?}");
    assert_eq!(seconds(&[ogg(&vorbis, 22_050), page(4, 22_050 * 9, b"")].concat()), Ok((1, 9.0)));
    let opus = [&b"OpusHead\x01\x02"[..], &312u16.to_le_bytes(), &48_000u32.to_le_bytes(), &[0; 3]].concat();
    assert_eq!(seconds(&ogg(&opus, 48_000 * 3 + 312)), Ok((2, 3.0)));
    assert!(Clip::probe("a.mp3", b"ID3 not a supported sound").is_err());
    assert!(
        playable("sounds/Wind.OGG")
            && playable("a.wav")
            && playable("a.mp3")
            && !playable("a.aac")
            && !playable("ogg")
    );
}

#[test]
fn mp3_clips_report_their_length_and_long_ones_need_streaming() {
    let frame = |head: [u8; 4], size: usize| [&head[..], &vec![0; size - 4]].concat();
    let stereo = frame([0xff, 0xfb, 0x90, 0x00], 417);
    let cbr = [&b"ID3\x04\0\0\0\0\0\0"[..], &stereo.repeat(100)].concat();
    let clip = Clip::read("a.mp3", &cbr).unwrap();
    assert_eq!((clip.rate, clip.channels, (clip.seconds * 100.0).round() / 100.0), (44_100, 2, 2.61));
    let mono = frame([0xff, 0xf3, 0x80, 0xc0], 208).repeat(50);
    let clip = Clip::probe("a.mp3", &mono).unwrap();
    assert_eq!((clip.rate, clip.channels, (clip.seconds * 100.0).round() / 100.0), (22_050, 1, 1.31));
    let tagged = |frames: u32| {
        let mut first = stereo.clone();
        first[36..44].copy_from_slice(b"Xing\0\0\0\x01");
        first[44..48].copy_from_slice(&frames.to_be_bytes());
        [first, stereo.clone()].concat()
    };
    let long = Clip::read("a.mp3", &tagged(4000)).unwrap();
    assert_eq!((long.seconds * 10.0).round() / 10.0, 104.5);
    assert!(Clip::probe("a.mp3", &tagged(4000)).unwrap_err().contains("at most 60 s"));
    assert!(Clip::read("a.mp3", &tagged(40_000)).unwrap_err().contains("at most 600 s"));
    assert!(Clip::read("a.mp3", &frame([0xff, 0xfb, 0xf0, 0x00], 417)).unwrap_err().contains("MP3"));
}

#[test]
fn only_files_named_mp3_are_read_as_mp3_and_only_from_their_first_frame() {
    let frame = [&[0xff, 0xfb, 0x90, 0x00][..], &[0; 413]].concat();
    let song = frame.repeat(20);
    assert!(Clip::read("song.mp3", &song).is_ok());
    for name in ["song.ogg", "song.wav", "song"] {
        assert!(Clip::read(name, &song).unwrap_err().contains("named .mp3"), "{name}");
    }
    let noise: Vec<u8> = (0..5000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8 & 0x7f).collect();
    let hidden = [&noise[..], &[0xff, 0xfb, 0x90, 0x00], &noise].concat();
    assert!(Clip::read("rain.mp3", &hidden).is_err());
    let lone = [&frame[..], &noise[..64]].concat();
    assert!(Clip::read("rain.mp3", &lone).is_err());
}

#[test]
fn streamed_clips_are_measured_from_their_edges() {
    let whole = wav(8000, 1, 800_000);
    let clip = Clip::sample("a.wav", &whole[..1024], &[], whole.len() as u64).unwrap();
    assert_eq!(clip.seconds, 100.0);
    let vorbis = [&b"\x01vorbis\0\0\0\0\x01"[..], &22_050u32.to_le_bytes(), &[0; 14]].concat();
    let long = [page(2, 0, &vorbis), vec![0; 4096], page(4, 22_050 * 200, b"x")].concat();
    let (head, tail) = (&long[..1024], &long[long.len() - 1024..]);
    assert_eq!(Clip::sample("a.ogg", head, tail, long.len() as u64).unwrap().seconds, 200.0);
    let stereo = [&[0xff, 0xfb, 0x90, 0x00][..], &[0; 413]].concat();
    let cbr = stereo.repeat(4000);
    let sampled = Clip::sample("a.mp3", &cbr[..4096], &[], cbr.len() as u64).unwrap();
    assert_eq!(sampled, Clip::read("a.mp3", &cbr).unwrap());
}
