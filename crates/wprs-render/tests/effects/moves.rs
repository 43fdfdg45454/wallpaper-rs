use super::*;

#[test]
fn transform_moves_scales_turns_and_skews_inside_the_layer() {
    let fx = |params: Value| Fx::new("transform", params).input(spot()).background("#00ff00").shot();
    let Some(same) = fx(json!({})) else { return };
    assert_eq!(same.diff(&Shot(spot())), 0.0);
    let Some(moved) = fx(json!({ "offset": [0.25, 0] })) else { return };
    moved.check(&[((48, 16), [255; 3]), ((32, 16), [0; 3]), ((4, 16), [0, 255, 0])]);
    let Some(small) = fx(json!({ "scale": [0.5, 0.5] })) else { return };
    small.check(&[((32, 16), [255; 3]), ((2, 2), [0, 255, 0]), ((20, 16), [0; 3])]);
    let Some(turned) = fx(json!({ "angle": 45, "pivot": [0, 0] })) else { return };
    let Some(skewed) = fx(json!({ "skew": [30, 0] })) else { return };
    differ(&[&same, &moved, &small, &turned, &skewed], 0.5);
    Shot::sheet(&[moved, small, turned, skewed]).looks_like("transform");
}

#[test]
fn spin_turns_the_middle_and_leaves_the_edges() {
    let Some(spun) = Fx::new("spin", json!({ "angle": 90, "radius": 0.3 })).input(squares()).shot() else { return };
    let corners = [(0, 0), (63, 0), (0, 31), (63, 31)];
    let unchanged = corners.iter().all(|&(x, y)| spun.at(x, y) == Shot(squares()).at(x, y));
    assert!(unchanged && spun.diff(&Shot(squares())) > 2.0);
    let Some(wide) = Fx::new("spin", json!({ "angle": 90, "radius": 2 })).input(squares()).shot() else { return };
    differ(&[&spun, &wide], 1.0);
    Shot::sheet(&[spun, wide]).looks_like("spin");
}

#[test]
fn displacement_follows_normal_or_offset_maps_or_its_own_noise() {
    let map = RgbaImage::from_pixel(8, 8, Rgba([255, 128, 128, 255]));
    let fx = |params: Value| Fx::new("displace", params).input(spot()).image("map", map.clone(), true).shot();
    let Some(offset) = fx(json!({ "map": "map", "mode": 1, "strength": 6 })) else { return };
    offset.check(&[((38, 16), [255; 3]), ((30, 16), [0; 3])]);
    let Some(normal) = fx(json!({ "map": "map", "strength": 6 })) else { return };
    let Some(noise) = fx(json!({ "strength": 6 })) else { return };
    differ(&[&offset, &normal, &noise], 0.2);
    Shot::sheet(&[offset, normal, noise]).looks_like("displace");
}
