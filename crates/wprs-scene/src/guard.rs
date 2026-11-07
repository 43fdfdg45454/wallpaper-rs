use crate::effects::{Descriptor, PRELUDE};
use crate::{CONTRACT, effect_source};
use naga::Module;
use naga::valid::{Capabilities, ValidationFlags, Validator};

mod cell;

pub fn guarded(d: &Descriptor) -> Result<Module, String> {
    if !crate::verdicts::passed(d.key) {
        return Err("its shader was not checked".into());
    }
    let source = straightened(source(d));
    crate::shader::roomy(|| armed(crate::shader::compile(&source)?.0, &source))
}

pub fn straightened(source: String) -> String {
    let plain = "fn tap(uv: vec2<f32>) -> vec4<f32> {\n    return sample(input_tex, uv);\n}\n";
    source.replacen(plain, include_str!("straight.wgsl"), 1)
}

pub(crate) fn source(d: &Descriptor) -> String {
    format!("{CONTRACT}{}", &effect_source(&[d])[PRELUDE.len()..])
}

pub(crate) fn armed(mut module: Module, source: &str) -> Result<Module, String> {
    crate::fence::guard(&mut module);
    crate::guard::cell::cell(&mut module)?;
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::default());
    validator.validate(&module).map_err(|e| format!("its guarded shader: {}", e.emit_to_string(source)))?;
    Ok(module)
}
