use crate::effects::{Descriptor, FRAME_WGSL};
use naga::valid::{Capabilities, ValidationFlags, Validator};

pub const STAGES_WGSL: &str = include_str!("stages.wgsl");

const PARTICLE: &str = concat!(
    "@group(2) @binding(0) var<uniform> frame: Frame;\n",
    "const TAU: f32 = 6.2831853;\n",
    "const LUMA: vec3<f32> = vec3<f32>(0.2126, 0.7152, 0.0722);\n",
    include_str!("oklab.wgsl"),
    include_str!("noise.wgsl"),
    include_str!("dither.wgsl"),
    include_str!("stages.wgsl")
);

pub fn particle_shader(d: &Descriptor, sprite: &str) -> Result<naga::Module, String> {
    if !crate::verdicts::passed(d.key) {
        return Err("its shader was not checked".into());
    }
    let source = format!("{FRAME_WGSL}{PARTICLE}\n{}\n{sprite}", d.wgsl);
    crate::shader::roomy(|| {
        let mut module = crate::shader::compile(&source)?.0;
        crate::fence::guard(&mut module);
        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::default());
        validator
            .validate(&module)
            .map_err(|e| format!("its guarded shader: {}", e.emit_to_string(&source)))?;
        Ok(module)
    })
}
