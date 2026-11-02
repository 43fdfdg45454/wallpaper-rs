use super::*;
use wprs_scene::Tier;

fn world(light: Value, spin: bool) -> (Value, BTreeMap<&'static str, Vec<u8>>) {
    let ground = Forge::default()
        .material(ForgeMaterial { roughness: 1.0, ..ForgeMaterial::new("ground", [1.0; 4]) })
        .part("ground", ForgeShape::Plane, [0.0; 3], 6.0, 0)
        .glb();
    let block = Forge::default()
        .material(ForgeMaterial::new("block", [1.0, 1.0, 1.0, 1.0]))
        .part("block", ForgeShape::Cube, [0.0; 3], 0.6, 0)
        .glb();
    let turn = if spin { json!({ "keys": [[0, [0, 0, 0]], [1, [0, 0, 90]]] }) } else { json!([0, 0, 0]) };
    let layers = json!([light, { "id": "world", "type": "group", "space": "3d", "layers": [
        { "id": "ground", "type": "model", "model": "ground" },
        { "id": "block", "type": "model", "model": "block", "position": [0, 0.8, 1], "rotation": turn }] }]);
    (layers, BTreeMap::from([("ground", ground), ("block", block)]))
}

fn sun(shadows: bool) -> Value {
    json!({ "id": "sun", "type": "light", "light": "directional", "intensity": 3,
        "transform": { "tilt": [45, 0] }, "shadows": shadows })
}

fn lamp(kind: &str) -> Value {
    let mut l = json!({ "id": "sun", "type": "light", "light": kind, "intensity": 6, "radius": 4000, "z": 30,
        "transform": { "position": [32, 10] }, "shadows": true });
    if kind == "spot" {
        l["cone"] = json!([60, 120]);
    }
    l
}

fn dim(px: &[u8], x: u32, y: u32) -> u32 {
    rgb(px, x, y).iter().map(|c| u32::from(*c)).sum()
}

#[test]
fn a_directional_light_with_shadows_darkens_the_ground_behind_a_block() {
    let Some(r) = renderer() else { return };
    r.force_tier(Some(Tier::T3));
    let (plain, models) = world(sun(false), false);
    let lit = render(r, scene(json!({}), plain, models), Quality::Q3);
    let (shaded, models) = world(sun(true), false);
    let mut shot = shoot(r, scene(json!({}), shaded, models), &[]);
    let dark = shot.frame(r, 0.0, Quality::Q3);
    assert!(dim(&dark, 32, 35) * 2 < dim(&lit, 32, 35), "{:?} {:?}", rgb(&dark, 32, 35), rgb(&lit, 32, 35));
    assert_eq!(dim(&dark, 6, 58), dim(&lit, 6, 58), "the open ground stays lit");
    assert_eq!(shot.view.shadow_sides(), vec![(2048, 1)]);
}

#[test]
fn spot_and_point_lights_cast_shadows_with_their_own_maps() {
    let Some(r) = renderer() else { return };
    r.force_tier(Some(Tier::T3));
    for (kind, side) in [("spot", (1024, 1)), ("point", (512, 6))] {
        let (layers, models) = world(lamp(kind), false);
        let mut shot = shoot(r, scene(json!({}), layers, models), &[]);
        let px = shot.frame(r, 0.0, Quality::Q3);
        assert_eq!(shot.view.shadow_sides(), vec![side], "{kind}");
        let mut open = lamp(kind);
        open["shadows"] = json!(false);
        let (layers, models) = world(open, false);
        let bare = render(r, scene(json!({}), layers, models), Quality::Q3);
        let darker = (20..44).flat_map(|y| (20..44).map(move |x| (x, y)));
        let most = darker.map(|(x, y)| dim(&bare, x, y).saturating_sub(dim(&px, x, y))).max();
        assert!(most > Some(30), "{kind} {most:?}");
    }
}

#[test]
fn still_shadows_are_kept_and_drawn_again_only_while_a_caster_moves() {
    let Some(r) = renderer() else { return };
    r.force_tier(Some(Tier::T3));
    let (layers, models) = world(sun(true), false);
    let mut shot = shoot(r, scene(json!({}), layers, models), &[]);
    (0..4).for_each(|_| _ = shot.frame(r, 0.1, Quality::Q3));
    assert_eq!(shot.view.shadow_renders(), 1);
    let (layers, models) = world(sun(true), true);
    let mut shot = shoot(r, scene(json!({}), layers, models), &[]);
    (0..3).for_each(|_| _ = shot.frame(r, 0.1, Quality::Q3));
    assert!(shot.view.shadow_renders() >= 3);
}

#[test]
fn an_explicit_resolution_is_kept_within_the_cap_and_halved_at_the_lowest_quality() {
    let Some(r) = renderer() else { return };
    r.force_tier(Some(Tier::T3));
    let mut light = sun(true);
    light["shadow_resolution"] = json!(512);
    let (layers, models) = world(light, false);
    let mut shot = shoot(r, scene(json!({}), layers, models), &[]);
    shot.frame(r, 0.0, Quality::Q0);
    assert_eq!(shot.view.shadow_sides(), vec![(256, 1)]);
}
