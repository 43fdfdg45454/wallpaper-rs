use super::*;

fn gl() -> Option<Renderer> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    desc.backends = wgpu::Backends::GL;
    desc.flags.remove(wgpu::InstanceFlags::VALIDATION);
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()));
    let opened = adapter
        .map_err(|e| e.to_string())
        .and_then(|a| Renderer::with_adapter(instance, a).map_err(|e| e.to_string()));
    opened.inspect_err(|e| eprintln!("skipping the OpenGL ES test: {e}")).ok()
}

#[test]
fn scenes_that_blend_in_srgb_render_on_opengl_es() {
    let Some(r) = gl() else { return };
    let layers = json!([{ "id": "s", "type": "solid", "color": "#ffffff", "opacity": 0.5,
        "effects": [{ "effect": "tint", "params": { "color": "#ff0000" } }] }]);
    let canvas = json!({ "size": [100, 50], "background": "#000000", "blend_space": "srgb" });
    let pixels = Shot::new(&r, canvas, layers, &[]).pixels(&r);
    assert!(near(at(&pixels, 50, 25), [128, 0, 0], 3), "{:?}", at(&pixels, 50, 25));
}
