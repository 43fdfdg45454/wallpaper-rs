use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::LazyLock;
use wprs_client::{Error, explain};

pub const INVALID: &str = "org.freedesktop.DBus.Error.InvalidArgs";
pub const FAILED: &str = "org.freedesktop.DBus.Error.Failed";
static FIXTURES: LazyLock<Value> =
    LazyLock::new(|| serde_json::from_str(include_str!("fixtures.json")).unwrap());

macro_rules! arg {
    (str) => { String };
    (strs) => { Vec<String> };
    (u32s) => { Vec<u32> };
    (map) => { HashMap<String, String> };
    ($t:tt) => { $t };
}

macro_rules! pass {
    (str, $v:ident) => {
        &$v
    };
    (strs, $v:ident) => {
        &$v.iter().map(String::as_str).collect::<Vec<_>>()
    };
    (u32s, $v:ident) => {
        &$v
    };
    (map, $v:ident) => {
        $v.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    };
    ($t:tt, $v:ident) => {
        $v
    };
}

macro_rules! reply {
    (json $e:expr) => {
        $e.map_err(fail)
    };
    ($e:expr) => {
        $e.map(|r| r.out().to_string()).map_err(fail)
    };
}

pub fn invalid(message: impl Into<String>) -> Error {
    Error { name: INVALID.into(), message: message.into() }
}

pub fn fail(error: zbus::Error) -> Error {
    explain(error)
        .downcast::<Error>()
        .unwrap_or_else(|e| Error { name: FAILED.into(), message: e.to_string() })
}

pub fn carries_secret(iface: &str, member: &str, args: &[Value]) -> bool {
    let secret = args.get(3).and_then(Value::as_str).is_some_and(|s| !s.is_empty());
    iface.ends_with("Providers1") && (member == "SetSecret" || member == "Add" && secret)
}

pub fn fixture(iface: &str, member: &str) -> String {
    FIXTURES.get(format!("{iface}.{member}")).unwrap_or(&json!([])).to_string()
}

pub fn next<T: DeserializeOwned>(args: &mut impl Iterator<Item = Value>, name: &str) -> Result<T, Error> {
    serde_json::from_value(args.next().unwrap_or(Value::Null)).map_err(|e| invalid(format!("{name}: {e}")))
}

pub trait Out {
    fn out(self) -> Value;
}

macro_rules! out {
    ($($t:ty),*) => {$(impl Out for $t { fn out(self) -> Value { json!([self]) } })*};
}

out!(String, u64, HashMap<String, String>);

impl Out for () {
    fn out(self) -> Value {
        json!([])
    }
}

impl Out for (String, u64) {
    fn out(self) -> Value {
        json!([self.0, self.1])
    }
}
