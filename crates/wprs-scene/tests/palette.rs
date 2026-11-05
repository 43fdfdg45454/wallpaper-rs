use wprs_scene::{FALLBACK, contrast, hex, palette};

fn picture(parts: &[([u8; 3], usize)]) -> Vec<u8> {
    let pixel = |(c, n): &([u8; 3], usize)| std::iter::repeat_n([c[0], c[1], c[2], 255], *n);
    parts.iter().flat_map(pixel).flatten().collect()
}

#[test]
fn the_most_common_colours_of_a_cover_come_first() {
    let found =
        palette(&picture(&[([200, 30, 30], 2000), ([20, 40, 200], 1500), ([240, 220, 40], 596)])).unwrap();
    assert_eq!(found[..3].iter().map(|c| hex(*c)).collect::<Vec<_>>(), ["#c81e1e", "#1428c8", "#f0dc28"]);
    assert!(contrast(found[0], found[3]) >= 4.5, "{:?}", found[3]);
    assert!(found[4] == [1.0; 4] || found[4] == [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn the_text_colour_is_readable_over_the_primary_one() {
    let white = palette(&picture(&[([250, 250, 250], 4096)])).unwrap();
    assert_eq!((hex(white[3]), hex(white[4])), ("#000000".into(), "#000000".into()));
    let night = palette(&picture(&[([10, 10, 40], 3000), ([250, 240, 200], 1096)])).unwrap();
    assert_eq!(hex(night[3]), "#faf0c8", "a light second colour reads well over a dark cover");
    assert_eq!(hex(night[4]), "#ffffff");
    let grey = palette(&picture(&[([120, 120, 120], 3000), ([140, 140, 140], 1096)])).unwrap();
    assert!(contrast(grey[0], grey[3]) >= 4.5 && [hex(grey[3])] != [hex(grey[1])]);
    for p in [white, night, grey] {
        assert!(contrast(p[0], p[3]) >= 4.5 && contrast(p[0], p[4]) >= 4.5);
    }
}

#[test]
fn clear_pixels_are_left_out_and_nothing_gives_no_palette() {
    let mut pixels = picture(&[([0, 200, 0], 100)]);
    pixels.extend([255, 0, 0, 0].repeat(4000));
    assert_eq!(hex(palette(&pixels).unwrap()[0]), "#00c800");
    assert!(palette(&[0, 0, 0, 0]).is_none() && palette(&[]).is_none());
    assert!(FALLBACK.iter().all(|c| c[3] == 1.0) && contrast(FALLBACK[0], FALLBACK[3]) >= 4.5);
}
