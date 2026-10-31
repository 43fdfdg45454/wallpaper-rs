#[path = "../common/mod.rs"]
mod common;

use common::{archive, image, v2};
use image::ImageFormat;
use serde_json::{Value, json};
use std::fs;
use std::io::Cursor;
use wprs_format::{Error, Kind, Limits, Manifest, Platform, features, manifest_schema, read};

mod origins;
mod texts;

fn with(kind: Kind, more: Value) -> Manifest {
    let mut value = serde_json::to_value(v2(kind)).unwrap();
    value.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
    serde_json::from_value(value).unwrap()
}

fn field(result: Result<(), Error>) -> String {
    match result {
        Err(Error::Invalid { field, .. }) => field,
        other => panic!("accepted: {other:?}"),
    }
}

fn named(manifest: &Manifest) -> Vec<String> {
    features::facts(manifest, None, None).features.into_iter().collect()
}
