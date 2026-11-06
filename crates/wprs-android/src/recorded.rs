use crate::exports::{engine, on};
use jni::objects::{JObject, JString};
use jni::sys::{jboolean, jfloat, jlong};
use jni::{EnvUnowned, jni_mangle};

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn perf<'a>(_: EnvUnowned<'a>, _: JObject<'a>, record: jboolean) {
    crate::perf::record(record);
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn looped<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong, nanos: jlong, ms: jfloat) {
    on(&mut env, |_| {
        engine(handle, |e| {
            e.looped(nanos, ms);
            Ok(())
        })
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn performance<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>) -> JString<'a> {
    on(&mut env, |env| Ok(env.new_string(crate::perf::report::report())?))
}
