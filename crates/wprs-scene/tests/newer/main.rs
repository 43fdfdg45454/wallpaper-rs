use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::*;

mod fields;
mod schema;

const FULL: &str = include_str!("../newer.json");
const SAMPLE: &str = include_str!("../sample.json");

const EXPECTED: [&str; 35] = [
    "ambient_light",
    "animated_textures",
    "audio_ranges",
    "audio_response",
    "blend_modes2",
    "bloom",
    "camera",
    "capture",
    "clock_dates",
    "clock_values",
    "custom_effects",
    "fonts2",
    "hdr",
    "keep_frame",
    "layer_color",
    "layer_playback",
    "layer_textures",
    "light_textures",
    "lighting2d",
    "lights",
    "mask_strength",
    "parallax2",
    "parent",
    "relative_values",
    "shaping",
    "sound2",
    "tangents",
    "text_layout",
    "text_styles",
    "texture_options",
    "tilt",
    "timeline_events",
    "tracks",
    "video_textures",
    "volumetric_lights",
];

fn full() -> Scene {
    Scene::load(FULL, &BTreeMap::new()).unwrap_or_else(|e| panic!("{e}"))
}
