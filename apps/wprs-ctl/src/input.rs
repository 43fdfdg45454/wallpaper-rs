use crate::usage;
use anyhow::{Context, Result, ensure};
use percent_encoding::percent_decode_str;
use std::collections::HashMap;
use std::io::{self, BufRead, IsTerminal};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use wprs_client::{Client, explain};
use wprs_core::dbus::uri_scheme;

pub fn path_arg(text: &str) -> Result<String> {
    let local = || usage(format!("{text} is not a local file"));
    match uri_scheme(text).filter(|_| !Path::new(text).exists()) {
        Some(scheme) if scheme.eq_ignore_ascii_case("file") => {
            let rest = &text[scheme.len() + 1..];
            let path = rest.strip_prefix("//").map_or(rest, |r| r.strip_prefix("localhost").unwrap_or(r));
            let path = percent_decode_str(path).decode_utf8().map_err(|_| local())?;
            path.starts_with('/').then(|| path.into_owned()).ok_or_else(local)
        }
        Some(_) => Err(local()),
        None => std::path::absolute(text)?.into_os_string().into_string().map_err(|_| local()),
    }
}

pub fn item_ref(text: &str) -> Result<String> {
    if text.contains('/') || Path::new(text).exists() { path_arg(text) } else { Ok(text.into()) }
}

pub async fn resolve(client: &Client, items: &[String]) -> Result<Vec<String>> {
    let refs = items.iter().map(|item| item_ref(item)).collect::<Result<Vec<_>>>()?;
    client.library.resolve(&strs(&refs)).await.map_err(explain)
}

pub async fn one(client: &Client, item: &str) -> Result<String> {
    resolve(client, &[item.into()]).await?.pop().context("wprsd resolved no item")
}

pub fn strs(items: &[String]) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

pub fn pairs(items: &[String]) -> Result<HashMap<&str, &str>> {
    items
        .iter()
        .map(|item| item.split_once('=').ok_or_else(|| usage(format!("{item} is not KEY=VALUE"))))
        .collect()
}

pub fn secret(prompt: &str, stdin: bool) -> Result<String> {
    if io::stdin().is_terminal() {
        return Ok(if stdin { rpassword::read_password()? } else { rpassword::prompt_password(prompt)? });
    }
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    line.truncate(line.trim_end_matches(['\r', '\n']).len());
    Ok(line)
}

pub fn password(twice: bool) -> Result<String> {
    if !io::stdin().is_terminal() {
        let hint = "no terminal to ask for the key password; set WPRS_KEY_PASSWORD";
        return std::env::var("WPRS_KEY_PASSWORD").context(hint);
    }
    let password = rpassword::prompt_password("Key password: ")?;
    ensure!(
        !twice || rpassword::prompt_password("Repeat the password: ")? == password,
        "the passwords differ"
    );
    Ok(password)
}

pub fn confirm(question: &str) -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    eprint!("{question} [y/N] ");
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes"))
}

pub fn now() -> Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs().try_into()?)
}
