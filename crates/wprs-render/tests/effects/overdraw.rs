use super::*;

const STRETCH: &str = "@vertex fn vs_over(@builtin(vertex_index) i: u32) -> VertexOut {
    let k = i % 6u;
    let c = vec2<f32>(select(0.0, 1.0, k == 1u || k == 4u || k == 5u), select(0.0, 1.0, k == 2u || k == 3u || k == 5u));
    return VertexOut(grid_position(c), c + grid_uv(i) * 0.0);
}
@fragment fn fs_count(in: VertexOut) -> @location(0) vec4<f32> { return vec4<f32>(0.0, 0.0, 0.0, 1.0 / 64.0); }
@fragment fn fs_show(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(sample(input_tex, in.uv).a, 0.0, 0.0, 1.0);
}";

fn drawn(shot: &Shot) -> f32 {
    let linear = |v: u8| {
        let c = f32::from(v) / 255.0;
        if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    shot.0.pixels().map(|p| linear(p.0[0]) * 64.0).sum::<f32>() / (W * H) as f32
}

fn stretched(grid: u32) -> Value {
    json!({ "id": "over", "cost": {}, "targets": { "acc": { "scale": 1, "format": "rgba16f" } }, "passes": [
        { "entry": "fs_count", "vertex": "vs_over", "grid": [grid, grid], "blend": "add", "target": "acc" },
        { "entry": "fs_show", "inputs": ["target:acc"] }] })
}

#[test]
fn a_grid_that_stretches_its_cells_draws_each_pixel_a_bounded_number_of_times() {
    let units: Vec<f32> = [1, 8, 64]
        .map(|g| Descriptor::packaged("over", &stretched(g).to_string(), STRETCH).unwrap().units())
        .to_vec();
    assert!(units[0] >= 2.0 && units[1] >= 9.0 && units[2] == units[1], "{units:?}");
    for (g, most) in [(1, 1.2), (4, 9.5), (8, 9.5)] {
        let Some(shot) = Fx::new("over", json!({})).own(stretched(g), STRETCH).shot() else { return };
        let times = drawn(&shot);
        assert!(times > 0.5 && times <= most, "a grid of {g} draws each pixel {times} times on average");
    }
}

#[test]
fn bent_grids_still_reach_past_their_cells() {
    let wgsl = "@vertex fn vs_bend(@builtin(vertex_index) i: u32) -> VertexOut {
        let uv = grid_uv(i);
        return VertexOut(grid_position(uv + vec2<f32>(0.0, 0.25 * sin(uv.x * PI))), uv);
    }
    @fragment fn fs_bend(in: VertexOut) -> @location(0) vec4<f32> { return vec4<f32>(0.0, 0.0, 1.0, 1.0); }";
    let json = json!({ "id": "bend", "cost": {}, "passes": [{ "entry": "fs_bend", "vertex": "vs_bend", "grid": [16, 16] }] });
    let Some(shot) = Fx::new("bend", json!({})).input(solid(RED)).own(json, wgsl).shot() else { return };
    shot.check(&[((32, 2), [0, 0, 0]), ((32, 28), [0, 0, 255]), ((2, 2), [0, 0, 255])]);
}
