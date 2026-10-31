use crate::engine::FRAMES;
use crate::exports::on;
use crate::power::Power;
use crate::settings::{self, BACKEND};
use crate::{account, choice, depth, gpu, library, stage, store};
use jni::objects::{JIntArray, JObject, JString};
use jni::refs::Reference;
use jni::sys::{jboolean, jint};
use jni::{EnvUnowned, jni_mangle};
use serde_json::json;
use std::fs::File;
use std::os::fd::BorrowedFd;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Mutex, PoisonError};
use std::time::Instant;

static COUNTED: Mutex<Option<(Instant, u64)>> = Mutex::new(None);

fn on_battery() -> bool {
    !Power::now().charging
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn store<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, fd: jint, name: JString<'a>) -> JString<'a> {
    on(&mut env, |env| {
        let mut source = File::from(unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?);
        let (uid, kind) = store::store(&mut source, &name.try_to_string(env)?)?;
        let poster = store::poster_source(&uid)?;
        Ok(env.new_string(json!({ "uid": uid, "kind": kind, "poster": poster }).to_string())?)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn poster<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    item: JString<'a>,
    pixels: JIntArray<'a>,
    width: jint,
    size: JIntArray<'a>,
    audio: jboolean,
) {
    on(&mut env, |env| {
        let (mut argb, mut facts) = (vec![0; pixels.len(env)?], [0; 3]);
        pixels.get_region(env, 0, &mut argb)?;
        size.get_region(env, 0, &mut facts)?;
        let [w, h, millis] = facts.map(i32::unsigned_abs);
        store::poster(&item.try_to_string(env)?, &argb, width.unsigned_abs(), (w, h), (millis, audio))
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn library<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, request: JString<'a>) -> JString<'a> {
    on(&mut env, |env| {
        let answer = library::request(&request.try_to_string(env)?, on_battery());
        Ok(env.new_string(answer)?)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn account<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, request: JString<'a>) -> JString<'a> {
    on(&mut env, |env| {
        let answer = account::request(&request.try_to_string(env)?);
        Ok(env.new_string(answer)?)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn depth<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, request: JString<'a>) -> JString<'a> {
    on(&mut env, |env| {
        let answer = depth::request(&request.try_to_string(env)?);
        Ok(env.new_string(answer)?)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn commit<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, target: JString<'a>) -> jboolean {
    on(&mut env, |env| {
        let target = serde_json::from_value(json!(target.try_to_string(env)?))?;
        choice::commit(on_battery(), target)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn rotate<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, lock: jboolean) -> jboolean {
    on(&mut env, |_| choice::advance(on_battery(), lock))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn colors<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    preview: jboolean,
    lock: jboolean,
) -> JIntArray<'a> {
    on(&mut env, |env| {
        let Some(colors) = choice::colors(preview, lock)? else { return Ok(JIntArray::default()) };
        let array = JIntArray::new(env, colors.len())?;
        array.set_region(env, 0, &colors)?;
        Ok(array)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn setting<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    key: JString<'a>,
    value: JString<'a>,
) -> JString<'a> {
    on(&mut env, |env| {
        let key = key.try_to_string(env)?;
        let value = if value.is_null() { None } else { Some(value.try_to_string(env)?) };
        let current = settings::setting(&key, value.as_deref())?;
        if key == BACKEND && value.is_some() {
            gpu::reset();
        }
        Ok(env.new_string(current)?)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn diagnostics<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>) -> JString<'a> {
    on(&mut env, |env| {
        let (frames, now, power) = (FRAMES.load(Relaxed), Instant::now(), Power::now());
        let mut counted = COUNTED.lock().unwrap_or_else(PoisonError::into_inner);
        let elapsed = |at: Instant| now.duration_since(at).as_secs_f64().max(0.001);
        let fps = counted.map(|(at, before)| ((frames - before) as f64 / elapsed(at)).round());
        *counted = Some((now, frames));
        let [tier, quality, reason] = stage::report::level().unwrap_or_default();
        let report = json!({
            "gpu": gpu::adapter(), "backend": settings::chosen(BACKEND), "frames": frames, "fps": fps,
            "cap": power.fps(false), "charging": power.charging, "saver": power.saver, "thermal": power.thermal,
            "tier": tier, "quality": quality, "reason": reason, "measured": gpu::measured(), "timed": gpu::timed(), "flashes": stage::report::flashes(), "left_out": stage::report::left_out(),
            "particles": stage::report::particles(), "scripts": crate::acting::scripts(), "stamps": gpu::stamps(),
        });
        Ok(env.new_string(report.to_string())?)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn finish<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>) {
    on(&mut env, |_| {
        crate::engine::forget();
        crate::stage::report::finish();
        Ok(())
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn retry<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>) {
    on(&mut env, |_| {
        stage::report::retry();
        Ok(())
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn trim<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>) {
    on(&mut env, |_| {
        crate::engine::forget();
        gpu::trim();
        Ok(())
    })
}
