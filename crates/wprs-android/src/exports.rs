use crate::engine::Engine;
use crate::gpu;
use crate::power::Power;
use crate::tilt::{self, Reading};
use crate::{Error, Logcat, store, vault};
use jni::errors::LogErrorAndDefault;
use jni::objects::{JObject, JString};
use jni::sys::{jboolean, jfloat, jint, jlong};
use jni::{Env, EnvUnowned, jni_mangle};
use ndk::native_window::NativeWindow;
use std::ptr::with_exposed_provenance_mut;
use std::time::Instant;

pub(crate) fn on<'a, T: Default + 'a>(
    env: &mut EnvUnowned<'a>,
    work: impl FnOnce(&mut Env<'a>) -> Result<T, Error>,
) -> T {
    env.with_env(work).resolve::<LogErrorAndDefault>()
}

pub(crate) fn engine<T: Default>(
    handle: jlong,
    work: impl FnOnce(&mut Engine) -> Result<T, Error>,
) -> Result<T, Error> {
    unsafe { with_exposed_provenance_mut::<Engine>(handle as usize).as_mut() }.map_or(Ok(T::default()), work)
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn prepare<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, files: JString<'a>) {
    if log::set_logger(&Logcat).is_ok() {
        crate::heif::register();
        log::set_max_level(log::LevelFilter::Info);
        std::panic::set_hook(Box::new(|info| log::error!("{info}")));
    }
    on(&mut env, |env| {
        vault::remember(env)?;
        store::prepare(&files.try_to_string(env)?)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn attach<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    surface: JObject<'a>,
    preview: jboolean,
    lock: jboolean,
    shown: jboolean,
) -> jlong {
    on(&mut env, |env| {
        let window = unsafe { NativeWindow::from_surface(env.get_raw().cast(), surface.as_raw().cast()) };
        let engine = Box::new(Engine::open(window.ok_or(Error::Window)?, preview, lock, shown));
        Ok(Box::into_raw(engine).expose_provenance() as jlong)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn resize<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong, width: jint, height: jint) {
    on(&mut env, |_| {
        engine(handle, |e| {
            e.resize(width, height);
            Ok(())
        })
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn offset<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong, x: jfloat) -> jboolean {
    on(&mut env, |_| engine(handle, |e| Ok(e.offset(x))))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn draw<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    handle: jlong,
    nanos: jlong,
    force: jboolean,
) -> jlong {
    on(&mut env, |_| engine(handle, |e| e.draw(nanos, force)))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn video<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) -> JString<'a> {
    on(&mut env, |env| match engine(handle, |e| Ok(e.video()))? {
        Some(uri) => Ok(env.new_string(uri)?),
        None => Ok(JString::default()),
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn detach<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) {
    on(&mut env, |_| {
        let engine = with_exposed_provenance_mut::<Engine>(handle as usize);
        if !engine.is_null() {
            drop(unsafe { Box::from_raw(engine) });
            gpu::trim();
        }
        Ok(())
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn power<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    charging: jboolean,
    saver: jboolean,
    low: jboolean,
    thermal: jint,
    headroom: jfloat,
    still: jboolean,
) {
    on(&mut env, |_| {
        Power { charging, saver, low, thermal, headroom, still }.set();
        Ok(())
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn moving<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>) -> jboolean {
    on(&mut env, |_| Ok(Power::now().moving()))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn parallax<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) -> jint {
    on(&mut env, |_| engine(handle, |e| Ok(e.parallax() as jint)))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn tilt<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    x: jfloat,
    y: jfloat,
    z: jfloat,
    rotation: jint,
) -> jboolean {
    on(&mut env, |_| Ok(tilt::sense(Reading::Gravity([x, y, z]), rotation, Instant::now())))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn turn<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    w: jfloat,
    x: jfloat,
    y: jfloat,
    z: jfloat,
    rotation: jint,
) -> jboolean {
    on(&mut env, |_| Ok(tilt::sense(Reading::Turn([w, x, y, z]), rotation, Instant::now())))
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn still<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>) {
    on(&mut env, |_| {
        tilt::forget();
        Ok(())
    })
}
