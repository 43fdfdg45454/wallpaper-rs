use crate::power::Power;
use crate::{depth, settings};
use serde_json::{Value, json};
use std::path::Path;

pub(super) fn layered(uid: &str, image: &Path, deep: &dyn Fn(Value) -> Value) {
    let item = || deep(json!({ "op": "item", "uid": uid }));
    let set = |key: &str, value: &str| settings::setting(key, Some(value));
    assert!(set("parallax.fill", "gentle").is_err() && set("parallax.layers", "9").is_err());
    set("parallax.layers", "4").unwrap();
    set("parallax.fill", "simple").unwrap();
    assert_eq!(deep(json!({ "op": "layers", "uid": uid })), Value::Null);
    assert_eq!(item()["layers"], json!(true));
    assert!(depth::raised(image).is_some_and(|raise| raise.deck.is_some()));
    set("parallax.fill", "off").unwrap();
    assert_eq!(item()["layers"], json!(false));
    assert!(deep(json!({ "op": "layers", "uid": uid }))["error"].as_str().unwrap().contains("fill is off"));
    set("parallax.fill", "lama").unwrap();
    let saver = |on: bool| Power { saver: on, ..Power::now() }.set();
    saver(true);
    assert_eq!(item()["fill"]["used"], json!("simple"), "LaMa never runs on battery saver");
    saver(false);
    assert_eq!(item()["fill"]["used"], json!("lama"));
    assert_eq!(item()["fill"]["chosen"], json!("lama"));
    set("parallax.fill", "simple").unwrap();
}

#[test]
fn lama_gives_way_to_the_simple_fill_on_battery_saver() {
    use wprs_core::Fill;
    assert_eq!(depth::layers::fill(Fill::Lama, true), Some(Fill::Simple));
    assert_eq!(depth::layers::fill(Fill::Lama, false), Some(Fill::Lama));
    assert_eq!(depth::layers::fill(Fill::Simple, true), Some(Fill::Simple));
    assert_eq!(depth::layers::fill(Fill::Off, false), None);
    assert_eq!(wprs_core::Config::default().parallax.layers, 5);
}
