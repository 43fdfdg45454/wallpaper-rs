use serde_json::json;
use wprs_scene::*;

mod hidden;
mod ops;
mod spill;
mod tables;

pub(crate) fn pack(wgsl: &str) -> Result<Descriptor, String> {
    let json = json!({ "id": "p", "cost": {}, "passes": [{ "entry": "fs_p" }] }).to_string();
    Descriptor::packaged("p", &json, wgsl)
}

pub(crate) fn small_stack(wgsl: String) -> Result<Descriptor, String> {
    let checked = std::thread::Builder::new().stack_size(2 << 20).spawn(move || pack(&wgsl)).unwrap();
    checked.join().expect("the check returns instead of crashing")
}

pub(crate) fn entry(body: &str) -> String {
    format!("@fragment fn fs_p(in: VertexOut) -> @location(0) vec4<f32> {{ {body} }}")
}

pub(crate) fn refused(wgsl: String, problem: &str) {
    let size = wgsl.len();
    let fault = small_stack(wgsl).err().unwrap_or_else(|| panic!("a {size} byte shader was accepted"));
    assert!(fault.contains(problem), "{size} bytes: {fault}");
}
