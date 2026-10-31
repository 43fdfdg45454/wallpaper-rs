use crate::said;
use crate::{Error, store};
use std::fs;
use wprs_sync::Secrets;
use zeroize::{Zeroize, Zeroizing};

const FILE: &str = "account.vault";
const TEXTS: &str = "texts.vault";

#[cfg(target_os = "android")]
mod keystore {
    use crate::{Error, said};
    use jni::objects::{JByteArray, JClass};
    use jni::refs::Global;
    use jni::{Env, JavaVM, jni_sig, jni_str};
    use std::sync::OnceLock;
    use zeroize::Zeroizing;

    static VAULT: OnceLock<Global<JClass<'static>>> = OnceLock::new();

    pub(crate) fn remember(env: &mut Env) -> Result<(), Error> {
        let class = env.find_class(jni_str!("io/github/wallpaper_rs/Vault"))?;
        let _ = VAULT.set(env.new_global_ref(class)?);
        Ok(())
    }

    pub(super) fn call(seal: bool, bytes: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
        let class = VAULT.get().ok_or(Error::Unprepared)?;
        JavaVM::singleton()?.attach_current_thread(|env| -> Result<_, Error> {
            let given = env.byte_array_from_slice(bytes)?;
            let name = if seal { jni_str!("seal") } else { jni_str!("open") };
            let found = env.call_static_method(class, name, jni_sig!("([B)[B"), &[(&given).into()]);
            if found.is_err() {
                env.exception_clear();
            }
            given.set_region(env, 0, &vec![0; bytes.len()])?;
            let refused = |_| {
                said(
                    "android.android_keystore_refused",
                    "the Android keystore refused the stored server secrets",
                    &[],
                )
            };
            let found = env.cast_local::<JByteArray>(found.map_err(refused)?.l()?)?;
            let out = Zeroizing::new(env.convert_byte_array(&found)?);
            found.set_region(env, 0, &vec![0; out.len()])?;
            Ok(out)
        })
    }
}

#[cfg(target_os = "android")]
pub(crate) use keystore::remember;

#[cfg(target_os = "android")]
fn seal(plain: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
    keystore::call(true, plain)
}

#[cfg(target_os = "android")]
fn open(sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
    keystore::call(false, sealed)
}

#[cfg(not(target_os = "android"))]
fn seal(plain: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
    Ok(Zeroizing::new(plain.to_vec()))
}

#[cfg(not(target_os = "android"))]
fn open(sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, Error> {
    Ok(Zeroizing::new(sealed.to_vec()))
}

pub(crate) fn load() -> Result<Secrets, Error> {
    let sealed = fs::read(store::dir()?.join(FILE)).map_err(|_| {
        said("android.phone_holds_token", "this phone holds no token for the server; link it again", &[])
    })?;
    let broken = || {
        said(
            "android.stored_server_secrets",
            "the stored server secrets are broken; remove the server and link again",
            &[],
        )
    };
    serde_json::from_slice(&open(&sealed).map_err(|_| broken())?).map_err(|_| broken())
}

pub(crate) fn store(secrets: &Secrets) -> Result<(), Error> {
    let plain = Zeroizing::new(serde_json::to_vec(secrets)?);
    store::write(&store::dir()?.join(FILE), &seal(&plain)?)
}

pub(crate) fn kept() -> std::collections::BTreeMap<String, String> {
    let sealed = store::dir().ok().and_then(|dir| fs::read(dir.join(TEXTS)).ok());
    let plain = sealed.and_then(|sealed| open(&sealed).ok());
    plain.and_then(|plain| serde_json::from_slice(&plain).ok()).unwrap_or_default()
}

pub(crate) fn keep(key: &str, text: &str) -> Result<(), Error> {
    let mut texts = kept();
    match text {
        "" => drop(texts.remove(key)),
        text => drop(texts.insert(key.to_owned(), text.to_owned())),
    }
    let plain = Zeroizing::new(serde_json::to_vec(&texts)?);
    texts.values_mut().for_each(Zeroize::zeroize);
    store::write(&store::dir()?.join(TEXTS), &seal(&plain)?)
}

pub(crate) fn forget() -> Result<(), Error> {
    match fs::remove_file(store::dir()?.join(FILE)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}
