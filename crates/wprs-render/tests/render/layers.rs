use super::*;

#[test]
fn fit_letterboxes_with_background() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255]));
    let pixels = draw(r, Some(&img), Fit::Fit, [0, 0, 255], (40, 20));
    assert!(close(rgb(&pixels, 40, 20, 10), [255, 0, 0]));
    assert!(close(rgb(&pixels, 40, 2, 10), [0, 0, 255]));
    assert!(close(rgb(&pixels, 40, 37, 10), [0, 0, 255]));
}

#[test]
fn fill_covers_the_target() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_fn(20, 10, |x, _| Rgba(if x < 10 { [0, 255, 0, 255] } else { [255; 4] }));
    let pixels = draw(r, Some(&img), Fit::Fill, [0, 0, 0], (64, 32));
    assert!(close(rgb(&pixels, 64, 4, 16), [0, 255, 0]));
    assert!(close(rgb(&pixels, 64, 60, 16), [255, 255, 255]));
    assert!(close(rgb(&pixels, 64, 4, 0), [0, 255, 0]));
}

#[test]
fn upload_downscales_only_when_needed() {
    let Some(r) = renderer() else { return };
    let size = |w, h, fit, target| r.upload(&RgbaImage::new(w, h), fit, target).unwrap().size();
    assert_eq!(size(100, 100, Fit::Fill, (200, 200)), (100, 100));
    assert_eq!(size(390, 390, Fit::Fill, (200, 200)), (390, 390));
    assert_eq!(size(4000, 2000, Fit::Fit, (400, 400)), (400, 200));
    assert_eq!(size(r.max_edge() * 2, 4, Fit::Center, (1000, 1000)), (r.max_edge(), 2));
}

#[test]
fn crossfade_blends_layers() {
    let Some(r) = renderer() else { return };
    let red = r.upload(&RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 255])), Fit::Fill, (8, 8)).unwrap();
    let blue = r.upload(&RgbaImage::from_pixel(4, 4, Rgba([0, 0, 255, 255])), Fit::Fill, (8, 8)).unwrap();
    let mut target = r.create_target(8, 8).unwrap();
    let mut pixels = Vec::new();
    let mut mix = |opacity| {
        let layers =
            [layer(Some(&red), Fit::Fill, [0; 3], 1.0), layer(Some(&blue), Fit::Fill, [0; 3], opacity)];
        r.render_layers(&mut target, &layers, &mut pixels).unwrap();
        rgb(&pixels, 8, 4, 4)
    };
    assert!(close(mix(0.0), [255, 0, 0]));
    assert!(close(mix(1.0), [0, 0, 255]));
    let is_linear_light_half = |channel: u8| channel.abs_diff(188) <= 3;
    let mid = mix(0.5);
    assert!(is_linear_light_half(mid[0]) && is_linear_light_half(mid[2]), "{mid:?}");
    assert!(mid[1] <= 2);
}

#[test]
fn video_frames_update_in_place() {
    let Some(r) = renderer() else { return };
    let green = [0u8, 255, 0, 255].repeat(6 * 4);
    let mut tex = r.upload_rgba(&green, 6, 4, 24).unwrap();
    let mut target = r.create_target(6, 4).unwrap();
    let mut pixels = Vec::new();
    let mut show = |tex: &GpuImage| {
        r.render_layers(&mut target, &[layer(Some(tex), Fit::Stretch, [0; 3], 1.0)], &mut pixels).unwrap();
        rgb(&pixels, 6, 3, 2)
    };
    assert!(close(show(&tex), [0, 255, 0]));

    let mut white_with_padded_rows = vec![0u8; 32 * 4];
    white_with_padded_rows.as_chunks_mut::<32>().0.iter_mut().for_each(|row| row[..24].fill(255));
    r.update_rgba(&mut tex, &white_with_padded_rows, 6, 4, 32).unwrap();
    assert_eq!(tex.size(), (6, 4));
    assert!(close(show(&tex), [255, 255, 255]));

    let red = [255u8, 0, 0, 255].repeat(3 * 2);
    r.update_rgba(&mut tex, &red, 3, 2, 12).unwrap();
    assert_eq!(tex.size(), (3, 2));
    assert!(close(show(&tex), [255, 0, 0]));
    assert!(r.update_rgba(&mut tex, &red[..8], 3, 2, 12).is_err());
}

#[test]
fn yuv_frames_convert_to_rgb_in_place() {
    let Some(r) = renderer() else { return };
    let frame = |left: [u8; 3], right: [u8; 3]| {
        let luma: Vec<u8> =
            (0..4).flat_map(|_| [left[0]; 3].into_iter().chain([right[0]; 3]).chain([0; 2])).collect();
        let row = [left[1], left[2], 128, 128, right[1], right[2], 0, 0];
        (luma, row.repeat(2))
    };
    let (red, green) = ([63, 102, 240], [173, 42, 26]);
    let mut slot = None;
    for (left, right, want) in
        [(red, green, [[255, 0, 0], [0, 255, 0]]), (green, red, [[0, 255, 0], [255, 0, 0]])]
    {
        let (luma, chroma) = frame(left, right);
        let planes =
            Planes { size: (6, 4), luma: (&luma, 8), chroma: (&chroma, 8), bt709: true, full: false };
        let image = r.read_image(r.convert_yuv(&mut slot, &planes).unwrap()).unwrap();
        let pixel = |x, y| -> [u8; 3] { image.get_pixel(x, y).0[..3].try_into().unwrap() };
        assert!(
            close(pixel(0, 0), want[0]) && close(pixel(5, 3), want[1]),
            "{:?} {:?}",
            pixel(0, 0),
            pixel(5, 3)
        );
    }
    let short = [0; 10];
    let planes = Planes { size: (6, 4), luma: (&short, 8), chroma: (&short, 8), bt709: false, full: true };
    assert!(r.convert_yuv(&mut slot, &planes).is_err());
}
