use crate::Error;
use crate::said;
use crate::state::{read, update};
use serde_json::{Value, json};
use wprs_sync::{Api, check_url, peek, valid_pin};

pub(crate) async fn add(url: &str, fingerprint: Option<&str>) -> Result<Value, Error> {
    let url = check_url(url)?;
    if read(|s| s.device.is_some()) {
        return Err(said(
            "android.phone_linked_server",
            "this phone is linked to a server; remove it first",
            &[],
        ));
    }
    let pin = fingerprint
        .map(|f| f.chars().filter(char::is_ascii_hexdigit).collect::<String>().to_ascii_lowercase());
    if pin.as_deref().is_some_and(|p| !valid_pin(p)) {
        return Err(said("android.fingerprint_digits", "a certificate fingerprint has 64 hex digits", &[]));
    }
    let found = match Api::pinned(&url, pin.as_deref())?.get("/.well-known/wprs-server").await {
        Err(wprs_sync::Error::Untrusted) if pin.is_none() => {
            let seen = peek(&url).await?;
            return Err(said(
                "android.server_own_certificate",
                "this server uses its own certificate with the fingerprint %1; add it only if the server shows the same one",
                &[&seen],
            ));
        }
        Err(wprs_sync::Error::Untrusted) => {
            return Err(said(
                "android.server_other_certificate",
                "the server's certificate is not the one with the fingerprint given",
                &[],
            ));
        }
        found => found?,
    };
    if !found["api"].as_array().is_some_and(|api| api.contains(&json!("v1"))) {
        return Err(said(
            "android.address_does_serve",
            "this address does not serve a wallpaper-rs server",
            &[],
        ));
    }
    let name: String =
        found["name"].as_str().unwrap_or_default().chars().filter(|c| !c.is_control()).take(128).collect();
    update(|s| (s.url, s.name, s.pin) = (url, name.clone(), pin))?;
    Ok(json!({"name": name, "registration": found["registration"]}))
}
