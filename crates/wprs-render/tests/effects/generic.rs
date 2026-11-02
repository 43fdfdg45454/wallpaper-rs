use super::*;

#[test]
fn unpainted_masks_and_generic_textures_read_their_defaults() {
    let wgsl = "@fragment fn fs_paint(in: VertexOut) -> @location(0) vec4<f32> {
    return select(sample(aux1, in.uv), sample(aux0, in.uv), in.uv.x < 0.5);
}";
    let json = json!({ "id": "paint", "cost": {}, "params": [
        { "id": "cover", "type": "mask", "default": "", "paint": "#ff0000" },
        { "id": "flat", "type": "texture", "default": "normal_flat" }],
        "passes": [{ "entry": "fs_paint", "inputs": ["previous", "asset:cover", "asset:flat"] }] });
    let Some(shot) = Fx::new("paint", json!({})).own(json, wgsl).shot() else { return };
    let mid = super::package::enc(128.0 / 255.0);
    shot.check(&[((8, 16), [255, 0, 0]), ((56, 16), [mid, mid, 255])]);
}

#[test]
fn a_use_names_a_generic_picture_instead_of_an_asset() {
    let wgsl = "@fragment fn fs_phase(in: VertexOut) -> @location(0) vec4<f32> {
    return select(sample(aux1, in.uv), sample(aux0, in.uv), in.uv.x < 0.5);
}";
    let json = json!({ "id": "phase", "cost": {}, "params": [
        { "id": "phase", "type": "texture", "default": "" },
        { "id": "flat", "type": "texture", "default": "normal_flat" }],
        "passes": [{ "entry": "fs_phase", "inputs": ["previous", "asset:phase", "asset:flat"] }] });
    let params = json!({ "phase": "white", "flat": "black" });
    let Some(shot) = Fx::new("phase", params).own(json, wgsl).shot() else { return };
    shot.check(&[((8, 16), [255, 255, 255]), ((56, 16), [0, 0, 0])]);
}
