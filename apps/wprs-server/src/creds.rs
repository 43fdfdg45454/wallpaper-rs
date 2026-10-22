use crate::App;
use crate::config::Config;
use crate::db::now;
use crate::error::{Problem, Res, require};
use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the system random source failed");
    bytes
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn digest(text: impl AsRef<[u8]>) -> Vec<u8> {
    Sha256::digest(text).to_vec()
}

pub fn token(kind: char) -> String {
    let body = hex(&random::<32>());
    format!("wprs_{kind}_{body}_{:08x}", crc32(body.as_bytes()))
}

pub fn well_formed(token: &str) -> bool {
    let parts: Vec<&str> = token.split('_').collect();
    let sum =
        |body: &str, sum: &str| sum.len() == 8 && u32::from_str_radix(sum, 16) == Ok(crc32(body.as_bytes()));
    let shaped = |kind: &str, body: &str| kind.len() == 1 && body.len() == 64;
    matches!(parts.as_slice(), ["wprs", kind, body, check] if shaped(kind, body) && sum(body, check))
}

fn crc32(data: &[u8]) -> u32 {
    let step = |crc: u32| (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
    !data.iter().fold(!0u32, |crc, &byte| (0..8).fold(crc ^ u32::from(byte), |c, _| step(c)))
}

pub fn user_code() -> String {
    const LETTERS: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ";
    let code: String =
        random::<8>().iter().map(|b| LETTERS[usize::from(*b) % LETTERS.len()] as char).collect();
    format!("{}-{}", &code[..4], &code[4..])
}

pub async fn hash(app: &App, password: String) -> Res<String> {
    let _permit = app.hashing.acquire().await.map_err(Problem::internal)?;
    let hashed = tokio::task::spawn_blocking(move || {
        Argon2::default().hash_password(password.as_bytes()).map(|h| h.to_string())
    });
    hashed.await?.map_err(Problem::internal)
}

pub async fn verify(app: &App, stored: Option<String>, password: String) -> Res<bool> {
    let _permit = app.hashing.acquire().await.map_err(Problem::internal)?;
    let known = stored.is_some();
    let stored = stored.unwrap_or_else(|| decoy().clone());
    let ok = tokio::task::spawn_blocking(move || {
        Argon2::default().verify_password(password.as_bytes(), stored.as_str()).is_ok()
    });
    Ok(ok.await? && known)
}

fn decoy() -> &'static String {
    static DECOY: OnceLock<String> = OnceLock::new();
    DECOY.get_or_init(|| {
        Argon2::default().hash_password(&random::<16>()).map(|h| h.to_string()).unwrap_or_default()
    })
}

pub fn check_handle(handle: &str) -> Res<()> {
    let chars =
        handle.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    let first = handle.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit());
    require(
        (3..=32).contains(&handle.len()) && chars && first,
        "handle: 3 to 32 lowercase letters, digits, - or _",
    )
}

pub fn check_password(handle: &str, password: &str) -> Res<()> {
    require((10..=256).contains(&password.chars().count()), "password: 10 to 256 characters")?;
    require(
        !password.to_lowercase().contains(&handle.to_lowercase()),
        "password: must not contain the handle",
    )
}

pub fn session(conn: &Connection, user: i64) -> Res<String> {
    let value = hex(&random::<32>());
    let sql = "INSERT INTO sessions (id_hash, user_id, created_at, seen_at) VALUES (?1, ?2, ?3, ?3)";
    conn.execute(sql, params![digest(&value), user, now()])?;
    Ok(value)
}

pub fn cookie(cfg: &Config, value: &str, max_age: i64) -> String {
    let secure = if cfg.secure() { "; Secure" } else { "" };
    format!("{}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}", cfg.cookie())
}
