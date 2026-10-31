use super::*;

#[test]
fn background_only() {
    let Some(r) = renderer() else { return };
    let pixels = draw(r, None, Fit::Fill, [10, 120, 250], (33, 17));
    assert_eq!(pixels.len(), 33 * 17 * 4);
    assert!(close(rgb(&pixels, 33, 0, 0), [10, 120, 250]));
    assert!(close(rgb(&pixels, 33, 32, 16), [10, 120, 250]));
    assert_eq!(pixels[(5 * 33 + 5) * 4 + 3], 255);
}

#[test]
fn readback_strips_row_padding_and_keeps_row_order() {
    let Some(r) = renderer() else { return };
    let img = RgbaImage::from_fn(2, 2, |_, y| Rgba(if y == 0 { [255, 0, 0, 255] } else { [0, 0, 255, 255] }));
    for width in [1, 63, 64, 65, 100] {
        let pixels = draw(r, Some(&img), Fit::Stretch, [0, 0, 0], (width, 4));
        assert_eq!(pixels.len(), width as usize * 4 * 4);
        assert!((0..width).all(|x| close(rgb(&pixels, width, x, 0), [255, 0, 0])), "width {width}");
        assert!((0..width).all(|x| close(rgb(&pixels, width, x, 3), [0, 0, 255])), "width {width}");
    }
}

#[test]
fn targets_and_destinations_are_reused() {
    let Some(r) = renderer() else { return };
    let red = r.upload_rgba(&[255, 0, 0, 255].repeat(4), 2, 2, 8).unwrap();
    let blue = r.upload_rgba(&[0, 0, 255, 255].repeat(4), 2, 2, 8).unwrap();
    let mut target = r.create_target(65, 9).unwrap();
    let mut pixels = Vec::new();
    let mut frame = |image, fit| {
        r.render_layers(&mut target, &[layer(Some(image), fit, [0, 255, 0], 1.0)], &mut pixels).unwrap();
        (pixels.as_ptr(), rgb(&pixels, 65, 32, 4), rgb(&pixels, 65, 1, 4))
    };
    let (first, color, _) = frame(&red, Fit::Fill);
    assert!(close(color, [255, 0, 0]));
    let (second, color, _) = frame(&blue, Fit::Fill);
    assert_eq!(first, second);
    assert!(close(color, [0, 0, 255]));
    let (_, center, edge) = frame(&blue, Fit::Fit);
    assert!(close(center, [0, 0, 255]) && close(edge, [0, 255, 0]));
    let (_, center, _) = frame(&red, Fit::Fill);
    assert!(close(center, [255, 0, 0]));
}

#[test]
fn rejects_invalid_and_oversized_targets() {
    let Some(r) = renderer() else { return };
    assert!(r.create_target(0, 10).is_err());
    assert!(r.create_target(r.max_edge() + 1, 10).is_err());
    assert!(r.check_target(u32::MAX, u32::MAX).is_err());
    assert!(r.check_target(1, 1).is_ok() && r.check_target(8200, 8200).is_err());
    assert_eq!(r.limits().max_edge, r.max_edge());
}

#[test]
fn borrowed_readback_hands_out_padded_rows() {
    let Some(r) = renderer() else { return };
    let mut target = r.create_target(65, 3).unwrap();
    let layers = [layer(None, Fit::Fill, [0, 255, 0], 1.0)];
    let read = |w, h, stride, data: &[u8]| ((w, h, stride, data.len()), [data[642], data[641], data[640]]);
    let (geometry, center) = r.render_layers_with(&mut target, &layers, read).unwrap();
    assert_eq!(geometry, (65, 3, 512, 512 * 3));
    assert!(close(center, [0, 255, 0]));
}

#[test]
fn target_limits_count_padded_rows() {
    let limits = TargetLimits { max_edge: 4096, max_buffer_size: 4096 * 4 * 100 };
    assert!(limits.check(4096, 100).is_ok());
    assert!(limits.check(4096, 101).is_err());
    assert!(limits.check(0, 1).is_err() && limits.check(1, 0).is_err() && limits.check(4097, 1).is_err());
    let tiny = TargetLimits { max_edge: 64, max_buffer_size: 512 };
    assert!(tiny.check(1, 2).is_ok() && tiny.check(64, 2).is_ok());
    assert!(tiny.check(1, 3).is_err() && tiny.check(65, 1).is_err());
}
