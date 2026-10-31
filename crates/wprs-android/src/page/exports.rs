use crate::exports::{engine, on};
use jni::objects::{JObject, JString};
use jni::sys::jlong;
use jni::{EnvUnowned, jni_mangle};

fn text<'a>(env: &mut jni::Env<'a>, value: Option<String>) -> Result<JString<'a>, crate::Error> {
    Ok(match value {
        Some(text) => env.new_string(text)?,
        None => JString::default(),
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn page<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) -> JString<'a> {
    on(&mut env, |env| {
        let spec = engine(handle, |e| Ok(e.page(handle)))?;
        text(env, spec)
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn serve<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong, url: JString<'a>) -> JString<'a> {
    on(&mut env, |env| {
        let url = url.try_to_string(env)?;
        let reply = super::serve::serve(handle, &url).unwrap_or_else(|e| {
            log::warn!("cannot serve a file of the page: {e}");
            serde_json::json!({})
        });
        text(env, Some(reply.to_string()))
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn heard<'a>(
    mut env: EnvUnowned<'a>,
    _: JObject<'a>,
    handle: jlong,
    message: JString<'a>,
) -> JString<'a> {
    on(&mut env, |env| {
        let message = message.try_to_string(env)?;
        text(env, super::script::heard(handle, &message))
    })
}

#[jni_mangle("io.github.wallpaper_rs.Native")]
pub fn listen<'a>(mut env: EnvUnowned<'a>, _: JObject<'a>, handle: jlong) -> JString<'a> {
    on(&mut env, |env| text(env, super::script::listen(handle)))
}
