#![cfg(target_os = "android")]

mod engine;
mod gpu;

use engine::Engine;
use jni::errors::LogErrorAndDefault;
use jni::objects::{JObject, JString};
use jni::sys::{jboolean, jint, jlong};
use jni::{EnvUnowned, jni_mangle};
use ndk::native_window::NativeWindow;
use std::ffi::{CString, c_char, c_int};
use std::ptr::with_exposed_provenance_mut;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Jni(#[from] jni::errors::Error),
    #[error(transparent)]
    Render(#[from] wprs_render::RenderError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("not a supported image: {0}")]
    Image(String),
    #[error("the surface has no native window")]
    Window,
    #[error("the app storage is not known yet")]
    Unprepared,
}

#[link(name = "log")]
unsafe extern "C" {
    fn __android_log_write(priority: c_int, tag: *const c_char, text: *const c_char) -> c_int;
}

struct Logcat;

impl log::Log for Logcat {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata())
            && let Ok(text) = CString::new(record.args().to_string())
        {
            unsafe { __android_log_write(7 - record.level() as c_int, c"wprs".as_ptr(), text.as_ptr()) };
        }
    }

    fn flush(&self) {}
}

fn live<'a>(handle: jlong) -> Option<&'a mut Engine> {
    unsafe { with_exposed_provenance_mut::<Engine>(handle as usize).as_mut() }
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn prepare<'local>(mut env: EnvUnowned<'local>, _: JObject<'local>, files: JString<'local>) {
    if log::set_logger(&Logcat).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
        std::panic::set_hook(Box::new(|info| log::error!("{info}")));
    }
    env.with_env(|env| files.try_to_string(env).map(engine::prepare)).resolve::<LogErrorAndDefault>()
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn attach<'local>(mut env: EnvUnowned<'local>, _: JObject<'local>, surface: JObject<'local>) -> jlong {
    env.with_env(|env| {
        let window = unsafe { NativeWindow::from_surface(env.get_raw().cast(), surface.as_raw().cast()) };
        let engine = Box::new(Engine::open(window.ok_or(Error::Window)?)?);
        Ok::<_, Error>(Box::into_raw(engine).expose_provenance() as jlong)
    })
    .resolve::<LogErrorAndDefault>()
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn resize<'local>(
    mut env: EnvUnowned<'local>,
    _: JObject<'local>,
    handle: jlong,
    width: jint,
    height: jint,
) {
    env.with_env(|_| {
        if let Some(engine) = live(handle) {
            engine.resize(width, height);
        }
        Ok::<_, Error>(())
    })
    .resolve::<LogErrorAndDefault>()
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn draw<'local>(
    mut env: EnvUnowned<'local>,
    _: JObject<'local>,
    handle: jlong,
    force: jboolean,
) -> jboolean {
    env.with_env(|_| live(handle).map_or(Ok(false), |e| e.draw(force))).resolve::<LogErrorAndDefault>()
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn detach<'local>(mut env: EnvUnowned<'local>, _: JObject<'local>, handle: jlong) {
    env.with_env(|_| {
        let engine = with_exposed_provenance_mut::<Engine>(handle as usize);
        if !engine.is_null() {
            drop(unsafe { Box::from_raw(engine) });
        }
        Ok::<_, Error>(())
    })
    .resolve::<LogErrorAndDefault>()
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn store<'local>(mut env: EnvUnowned<'local>, _: JObject<'local>, fd: jint) -> jboolean {
    env.with_env(|_| engine::store(fd)).resolve::<LogErrorAndDefault>()
}
