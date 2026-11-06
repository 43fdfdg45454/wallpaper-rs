use super::styles::styled;
use super::*;

#[test]
fn styled_texts_look_the_same_on_cards_without_half_float_targets() {
    let Some(r) = renderer() else { return };
    let Ok(own) = Renderer::new() else { return };
    own.force_eight_bit(true);
    let both = json!({ "outline": { "color": "#ff0000", "width": 2 }, "shadow": { "color": "#00ff00", "offset": [4, 3], "blur": 2 },
        "text": "AxA", "render": "sdf" });
    let wide = styled(r, both.clone()).pixels(r);
    let mut shot = styled(&own, both);
    let narrow = shot.pixels(&own);
    let off = (0..100)
        .flat_map(|x| (0..50).map(move |y| (x, y)))
        .filter(|&(x, y)| !near(at(&wide, x, y), at(&narrow, x, y), 24));
    assert!(off.count() <= 25, "the field made on the processor looks like the one the card makes");
    looks_like("styles", &narrow);
    let ringed = &styled(&own, json!({ "outline": { "color": "#ff0000", "width": 3 } })).pixels(&own);
    let red =
        (0..100).flat_map(|x| (0..50).map(move |y| at(ringed, x, y))).filter(|p| p[0] > 200 && p[1] < 60);
    assert!(red.count() > 30, "the outline is drawn, not left out");
}
