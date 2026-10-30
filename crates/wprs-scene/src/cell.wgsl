
fn fx_cell(a: vec4<f32>, b: vec4<f32>, c: vec4<f32>, k: u32) -> vec4<f32> {
    let w = vec3<f32>(max(a.w, 1e-6), max(b.w, 1e-6), max(c.w, 1e-6));
    let pa = a.xy / w.x;
    let pb = b.xy / w.y;
    let pc = c.xy / w.z;
    let span = max(max(pa, max(pb, pc)) - min(pa, min(pb, pc)), vec2<f32>(1e-9));
    let reach = 6.0 / max(more.grid, vec2<f32>(1.0));
    let fit = min(1.0, min(reach.x / span.x, reach.y / span.y));
    let s = select(0.0, fit, fit == fit);
    let mid = (pa + pb + pc) / 3.0;
    let me = select(select(c, b, k == 1u), a, k == 0u);
    let own = select(select(w.z, w.y, k == 1u), w.x, k == 0u);
    return vec4<f32>((mid + (me.xy / own - mid) * s) * own, me.z, own);
}
