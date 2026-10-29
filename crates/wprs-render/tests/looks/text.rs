use super::fonts::{boxes, packed};
use super::*;
use ab_glyph::Font;

fn sans() -> Option<Vec<u8>> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let plain = |f: &&fontdb::FaceInfo| f.index == 0 && f.families.iter().any(|(n, _)| n.ends_with("Sans"));
    let ids: Vec<_> = db.faces().filter(plain).map(|f| f.id).collect();
    let data =
        |id| db.with_face_data(id, |data, _| data.starts_with(&[0, 1, 0, 0]).then(|| data.to_vec()))?;
    ids.into_iter().find_map(data)
}

fn collection(ttf: &[u8]) -> Vec<u8> {
    let at = 20u32;
    let mut out = [&b"ttcf\0\x01\0\0\0\0\0\x02"[..], &at.to_be_bytes(), &at.to_be_bytes()].concat();
    let mut font = ttf.to_vec();
    let tables = u16::from_be_bytes([font[4], font[5]]) as usize;
    for t in 0..tables {
        let o = 12 + t * 16 + 8;
        let moved = u32::from_be_bytes(font[o..o + 4].try_into().unwrap()) + at;
        font[o..o + 4].copy_from_slice(&moved.to_be_bytes());
    }
    out.extend(font);
    out
}

fn rows(pixels: &[u8], x: std::ops::Range<u32>, test: impl Fn([u8; 3]) -> bool) -> Vec<u32> {
    (0..50).filter(|&y| x.clone().any(|x| test(at(pixels, x, y)))).collect()
}

#[test]
fn fonts_load_from_collections_and_refuse_broken_or_huge_files() {
    let Some(ttf) = sans() else { return assert!(!fontdb_has_fonts(), "no sans-serif font") };
    let ttc = collection(&ttf);
    let face = wprs_render::font(&ttc, 1).expect("the second face of the collection");
    assert!(face.glyph_id('A').0 != 0);
    assert!(wprs_render::font(&ttc, 2).is_none() && wprs_render::font(b"nonsense", 0).is_none());
    let mut bomb = b"wOF2\0\x01\0\0\0\0\0\0\0\0\0\0".to_vec();
    bomb.extend((u32::MAX).to_be_bytes());
    assert!(wprs_render::font(&bomb, 0).is_none());
    assert!(wprs_render::font(&vec![0; wprs_render::MAX_FONT + 1], 0).is_none());
}

#[test]
fn compressed_fonts_read_like_their_source_and_refuse_bombs() {
    let ttf = boxes();
    let (plain, small) =
        (wprs_render::font(&ttf, 0).unwrap(), wprs_render::font(&packed(&ttf, None, 0), 0).unwrap());
    for c in ['A', 'g', '!', ' '] {
        let shape = |f: &ab_glyph::FontArc| {
            let id = f.glyph_id(c);
            format!("{id:?} {} {:?}", f.h_advance_unscaled(id), f.outline(id))
        };
        assert_eq!(shape(&plain), shape(&small), "{c}");
    }
    assert!(plain.glyph_id('A').0 != 0 && plain.outline(plain.glyph_id('A')).is_some());
    let huge = packed(&ttf, Some(0xffff_0000), 0);
    assert!(wprs_render::font(&huge, 0).is_none(), "tables longer than the stream");
    let short = packed(&ttf, Some(64), 0);
    assert!(wprs_render::font(&short, 0).is_none(), "a stream shorter than its tables");
    let zeros = packed(&ttf, None, wprs_render::MAX_FONT);
    assert!(
        zeros.len() < 64 << 10 && wprs_render::font(&zeros, 0).is_none(),
        "a stream that expands too far"
    );
}

#[test]
fn texts_wrap_cut_align_and_sit_on_their_box() {
    let Some(r) = renderer() else { return };
    let Some(ttf) = sans() else { return assert!(!fontdb_has_fonts(), "no sans-serif font") };
    let shot = |text: Value| {
        let scene = json!({ "canvas": canvas("#000000"), "assets": { "f": { "path": "f.ttc", "face": 1 } },
            "layers": [text] });
        let mut shot = Shot::with(r, scene.clone(), &[]);
        let fonts = [("f".to_string(), Arc::new(collection(&ttf)))].into();
        let assets = SceneAssets { fonts, ..Default::default() };
        shot.view = r.scene_view(&shot.runtime.scene, &assets, shot.runtime.props()).unwrap();
        shot.pixels(r)
    };
    let words = |more: Value| {
        let mut t = json!({ "id": "t", "type": "text", "text": "wide words wrap here", "font": "f", "size": 8,
            "color": "#ffffff", "transform": { "position": [50, 25] } });
        t.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
        rows(&shot(t), 0..100, |p| p[0] > 128)
    };
    let one = words(json!({}));
    let wrapped = words(json!({ "wrap": { "width": 30 } }));
    let cut = words(json!({ "wrap": { "width": 30, "lines": 2, "ellipsis": true } }));
    let spaced = words(json!({ "wrap": { "width": 30, "lines": 2 }, "line_height": 2 }));
    let span = |r: &[u32]| r.last().unwrap() - r.first().unwrap();
    assert!(span(&wrapped) > 2 * span(&one) && span(&cut) < span(&wrapped), "{one:?} {wrapped:?} {cut:?}");
    assert!(span(&spaced) > span(&cut) + 4, "{spaced:?} {cut:?}");
    let boxed = json!({ "id": "t", "type": "text", "text": "Hi", "font": "f", "size": 16, "color": "#ffffff",
        "valign": "top", "padding": 2, "transform": { "position": [50, 10] },
        "background": { "color": "#ff0000", "padding": [10, 4], "radius": 3 } });
    let pixels = shot(boxed);
    let red = rows(&pixels, 45..55, |p| p == [255, 0, 0]);
    assert!(red.first().is_some_and(|y| (12..15).contains(y)), "{red:?}");
    check(&pixels, &[((50, 9), [0, 0, 0]), ((5, 20), [0, 0, 0])]);
    let left = json!({ "id": "t", "type": "text", "text": "Hi\nHello there", "font": "f", "size": 10,
        "color": "#ffffff", "align": "right", "transform": { "position": [50, 25] } });
    let pixels = shot(left);
    let line =
        |y: std::ops::Range<u32>| (0..100).filter(|&x| y.clone().any(|y| at(&pixels, x, y)[0] > 128)).max();
    let ends = (line(14..25), line(26..37));
    assert!(ends.0.is_some() && ends.0.unwrap().abs_diff(ends.1.unwrap()) <= 2, "{ends:?}");
}

#[test]
fn texts_lay_out_like_their_reference_image() {
    let Some(r) = renderer() else { return };
    let layers = json!([
        { "id": "w", "type": "text", "text": "Wide words wrap here", "font": "b", "size": 8, "color": "#ffffff",
          "wrap": { "width": 40, "lines": 2, "ellipsis": true }, "line_height": 1.5, "transform": { "position": [25, 25] } },
        { "id": "r", "type": "text", "text": "Hi\nHello", "font": "b", "size": 7, "color": "#00ff00", "align": "right",
          "valign": "top", "transform": { "position": [70, 4] } },
        { "id": "k", "type": "text", "text": "Ok", "font": "b", "size": 10, "color": "#ffffff", "padding": 1,
          "transform": { "position": [80, 38] }, "background": { "color": "#ff0000", "padding": [4, 2], "radius": 2 } }
    ]);
    let scene =
        json!({ "canvas": canvas("#000000"), "assets": { "b": { "path": "b.ttf" } }, "layers": layers });
    let mut shot = Shot::with(r, scene, &[]);
    let assets = SceneAssets { fonts: [("b".to_string(), Arc::new(boxes()))].into(), ..Default::default() };
    shot.view = r.scene_view(&shot.runtime.scene, &assets, shot.runtime.props()).unwrap();
    looks_like("text", &shot.pixels(r));
}
