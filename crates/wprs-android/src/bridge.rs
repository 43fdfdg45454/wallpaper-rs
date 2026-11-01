use crate::exports::{engine, on};
use crate::hearing::{self, MAX_CAPTURE};
use crate::power;
use jni::objects::{JByteArray, JObject, JString};
use jni::sys::{jboolean, jfloat, jint, jlong};
use jni::{EnvUnowned, jni_mangle};
use std::path::Path;

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn device<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, low_ram: jboolean, class: jint) {
    on(&mut env, |_| {
        power::device(low_ram, class);
        Ok(())
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn screen<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong, lock: jboolean) {
    on(&mut env, |_| {
        engine(handle, |e| {
            e.screen(lock);
            Ok(())
        })
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn visible<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong, shown: jboolean) {
    on(&mut env, |_| {
        engine(handle, |e| {
            e.visible(shown);
            Ok(())
        })
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn inputs<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) -> jint {
    on(&mut env, |_| engine(handle, |e| Ok(e.inputs())))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn touch<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    handle: jlong,
    x: jfloat,
    y: jfloat,
    down: jboolean,
) -> jboolean {
    on(&mut env, |_| engine(handle, |e| Ok(e.touch(x, y, down))))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn hear<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, fft: JByteArray<'a>, rate: jint) {
    on(&mut env, |env| {
        let mut bytes = [0; MAX_CAPTURE];
        let n = fft.len(env)?.min(MAX_CAPTURE);
        fft.get_region(env, 0, &mut bytes[..n])?;
        hearing::hear(&bytes[..n], rate.unsigned_abs());
        Ok(())
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn playback<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, active: jboolean) {
    on(&mut env, |_| {
        hearing::playback(active);
        Ok(())
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn film<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) -> JString<'a> {
    on(&mut env, |env| match engine(handle, |e| Ok(e.film()))? {
        Some(path) => Ok(env.new_string(path)?),
        None => Ok(JString::default()),
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn clips<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) -> JString<'a> {
    on(&mut env, |env| match engine(handle, |e| Ok(e.clips()))? {
        Some(paths) => Ok(env.new_string(serde_json::to_string(&paths)?)?),
        None => Ok(JString::default()),
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn steer<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) -> JString<'a> {
    on(&mut env, |env| match engine(handle, |e| Ok(e.steer()))? {
        Some(list) => Ok(env.new_string(list)?),
        None => Ok(JString::default()),
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn clip<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    handle: jlong,
    path: JString<'a>,
    open: jboolean,
) -> JObject<'a> {
    on(&mut env, |env| {
        let path = path.try_to_string(env)?;
        let Some(window) = engine(handle, |e| e.clip(Path::new(&path), open))? else {
            return Ok(JObject::null());
        };
        let surface = unsafe { window.to_surface(env.get_raw().cast()) };
        Ok(unsafe { JObject::from_raw(env, surface.cast()) })
    })
}
