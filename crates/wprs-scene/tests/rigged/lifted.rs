use super::kit::layers;
use serde_json::{Value, json};
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 64.0, 64.0];
const FAR: f32 = 200.0;

fn tilted(group: [f32; 2], fox: Value) -> Runtime {
    let mut fox = fox;
    fox["id"] = json!("fox");
    (fox["type"], fox["mesh"], fox["size"]) = (json!("mesh"), json!("arm"), json!([64, 64]));
    fox["depth"] = json!({ "scale": 1 });
    let rt = layers(json!([{ "id": "g", "type": "group",
        "transform": { "position": [32, 32], "tilt": group }, "layers": [fox] }]));
    let mut scene = (*rt.scene).clone();
    scene.canvas.perspective = Some(FAR);
    let mut rt = Runtime::new(std::sync::Arc::new(scene), Default::default());
    rt.advance(0.1);
    rt
}

fn shown(rt: &Runtime, [x, y, z]: [f32; 3]) -> [f32; 2] {
    let frame = rt.frame(VIEW, Quality::Q3);
    let d = frame.draws.iter().find(|d| d.layer.id == "fox").unwrap();
    let Content::Mesh { size, relief: Some(r), .. } = &d.content else { panic!("{:?}", d.content) };
    let corner = Affine::translate([-d.origin[0] * size[0], -d.origin[1] * size[1]]);
    let boxed = Affine([1.0, 0.0, 0.0, -1.0, size[0] / 2.0, size[1] / 2.0]);
    let mesh = Lean::of(d.world.then(corner).then(boxed));
    let full = d.lean.map_or(mesh, |l| l.then(mesh));
    let p = [x + r.lift[0] * z, y + r.lift[1] * z, 1.0 + r.lift[2] * z];
    let h: [f32; 3] = std::array::from_fn(|i| (0..3).map(|j| full.0[i * 3 + j] * p[j]).sum());
    [h[0] / h[2], h[1] / h[2]]
}

fn turn(v: [f32; 3], [tx, ty]: [f32; 2]) -> [f32; 3] {
    let [(sx, cx), (sy, cy)] = [tx, ty].map(|a| a.to_radians().sin_cos());
    let (ex, ey, ez) = ([cy, 0.0, sy], [sx * sy, cx, -sx * cy], [-sy * cx, sx, cx * cy]);
    std::array::from_fn(|i| ex[i] * v[0] + ey[i] * v[1] + ez[i] * v[2])
}

fn seen(at: [f32; 3]) -> [f32; 2] {
    let w = 1.0 + at[2] / FAR;
    [32.0 + (at[0] - 32.0) / w, 32.0 + (at[1] - 32.0) / w]
}

fn reference(group: [f32; 2], own: [f32; 2], fox: [f32; 2], [x, y, z]: [f32; 3]) -> [f32; 3] {
    let local = turn([x, -y, -z], own);
    let inner = [fox[0] + local[0], fox[1] + local[1], local[2]];
    let o = [32.0, 32.0];
    let r = turn([inner[0] - o[0], inner[1] - o[1], inner[2]], group);
    [o[0] + r[0], o[1] + r[1], r[2]]
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) < 0.02
}

#[test]
fn raised_vertices_lean_with_the_tilted_group_around_their_layer() {
    for group in [[0.0, 30.0], [25.0, -20.0], [-40.0, 10.0]] {
        let rt = tilted(group, json!({ "transform": { "position": [6, -4] } }));
        for v in [[0.0, 0.0, 12.0], [10.0, -8.0, 20.0], [-14.0, 6.0, -9.0]] {
            let want = seen(reference(group, [0.0; 2], [38.0, 28.0], v));
            let flat = seen(reference(group, [0.0; 2], [38.0, 28.0], [v[0], v[1], 0.0]));
            let got = shown(&rt, v);
            assert!(near(got, want), "{group:?} {v:?}: {got:?} {want:?} (flat {flat:?})");
        }
    }
}

#[test]
fn a_layer_without_tilt_raises_straight_towards_the_viewer() {
    let rt = layers(json!([{ "id": "fox", "type": "mesh", "mesh": "arm", "size": [64, 64],
        "depth": { "scale": 1 }, "transform": { "position": [40, 30] } }]));
    let mut scene = (*rt.scene).clone();
    scene.canvas.perspective = Some(FAR);
    let mut rt = Runtime::new(std::sync::Arc::new(scene), Default::default());
    rt.advance(0.1);
    for v in [[0.0, 0.0, 12.0], [10.0, -8.0, -20.0]] {
        let want = seen([40.0 + v[0], 30.0 - v[1], -v[2]]);
        assert!(near(shown(&rt, v), want), "{v:?}: {:?} {want:?}", shown(&rt, v));
    }
}

#[test]
fn a_layer_tilted_on_its_own_raises_along_its_turned_face() {
    for own in [[20.0, -15.0], [-35.0, 40.0]] {
        let rt = tilted([0.0; 2], json!({ "transform": { "position": [6, -4], "tilt": own } }));
        for v in [[0.0, 0.0, 12.0], [10.0, -8.0, 20.0]] {
            let want = seen(reference([0.0; 2], own, [38.0, 28.0], v));
            assert!(near(shown(&rt, v), want), "{own:?} {v:?}: {:?} {want:?}", shown(&rt, v));
        }
    }
}
