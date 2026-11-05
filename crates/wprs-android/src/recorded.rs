use crate::exports::on;
use jni::objects::{JObject, JString};
use jni::sys::jboolean;
use jni::{EnvUnowned, jni_mangle};

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn perf<'a>(_: EnvUnowned<'a>, _: JObject<'a>, record: jboolean) {
    crate::perf::record(record);
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn performance<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>) -> JString<'a> {
    on(&mut env, |env| Ok(env.new_string(crate::perf::report())?))
}
