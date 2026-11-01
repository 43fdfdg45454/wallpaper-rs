package io.github.wallpaper_rs

const val TAPS_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 64] },
    "scripts": [{ "path": "tap.js", "events": ["init", "click"], "uses": ["layers", "clicks"] }],
    "timelines": [{ "id": "light", "duration": 0.5, "autoplay": false, "trigger": "click:button" }],
    "layers": [{ "id": "button", "type": "solid", "color": "#2040c0", "size": [32, 64], "solid": true, "transform": { "position": [16, 32] } },
        { "id": "lamp", "type": "solid", "color": "#ffffff", "size": [32, 64], "transform": { "position": [48, 32] },
          "opacity": { "timeline": "light", "keys": [[0, 0], [0.5, 1]] } }] }"""
const val TAPS_SCRIPT = """import { scene } from "wprs";
export function init() { scene.layer("button").on("click", () => { scene.layer("lamp").color = "#ff0000"; }); }
"""
const val TAPS_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.taps", "version": "1.0.0",
    "title": "Taps", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false, "inputs": ["pointer", "clicks"] }"""
const val CLIP_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
    "assets": { "clip": { "path": "clip.webm" } }, "layers": [{ "id": "clip", "type": "video", "video": "clip" }] }"""
const val CLIP_MANIFEST = """{ "schema": "wallpaper-rs.wpak/1", "id": "org.example.clip", "version": "1.0.0",
    "title": "Clip", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json" }"""
const val KNOBS_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
    "timelines": [{ "id": "glow", "duration": 1, "autoplay": false, "trigger": "prop:again" }],
    "layers": [{ "id": "fill", "type": "solid", "color": "#ff0000", "opacity": { "timeline": "glow", "keys": [[0, 0], [1, 1]] } }] }"""
const val KNOBS_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.knobs", "version": "1.0.0",
    "title": "Knobs", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false, "requires": { "features": ["folder", "properties2"] },
    "properties": { "mood": { "type": "combo", "default": 1, "options": [1, 2] }, "again": { "type": "button" },
                    "token": { "type": "text", "secret": true }, "photos": { "type": "folder" } } }"""
const val PAGE_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.page", "version": "1.0.0",
    "title": "Page", "license": "CC0-1.0", "kind": "web", "entry": "index.html", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false, "requires": { "features": ["web"] } }"""
const val LAMP_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
    "layers": [{ "id": "lamp", "type": "light", "light": "spot", "cone": [10, 30] }] }"""
const val LAMP_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.lamp", "version": "1.0.0",
    "title": "Lamp", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false, "requires": { "features": ["lights"] } }"""
const val STAGE_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] }, "camera3d": { "fov": 50 },
    "layers": [{ "id": "stage", "type": "group", "space": "3d", "layers": [] }] }"""
const val STAGE_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.stage", "version": "1.0.0",
    "title": "Stage", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false, "requires": { "features": ["camera3d", "model3d"] } }"""
const val SHAFTS_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
    "layers": [{ "id": "fill", "type": "solid", "color": "#ffffff",
                 "effects": [{ "effect": "glitter" }, { "effect": "blur", "params": { "blend": "screen" } }] }] }"""
const val SHAFTS_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.shafts", "version": "1.0.0",
    "title": "Shafts", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false,
    "requires": { "features": ["effect.blur.blend", "effect.glitter"] } }"""
const val MIXES_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] }, "layers": [
    { "id": "spot", "type": "shape", "shape": { "ellipse": { "size": [20, 20] } }, "fill": "#00ff00", "visible": false },
    { "id": "fill", "type": "solid", "color": "#ffffff", "blend": "linear_burn",
      "effects": [{ "effect": "light_shafts", "params": { "speed": 0 } },
                  { "effect": "tint", "mask": { "shape": { "ellipse": { "size": [30, 20] } } } },
                  { "effect": "blend_texture", "params": { "texture": { "layer": "spot" } }, "mask": { "layer": "spot" } },
                  { "effect": "glitter", "params": { "speed": 0 } }, { "effect": "trail" },
                  { "effect": "god_rays", "params": { "mode": 2 } }, { "effect": "fluid", "params": { "push": 0 } }] }] }"""
const val MIXES_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.mixes", "version": "1.0.0",
    "title": "Mixes", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false, "requires": { "features": ["blend_modes2",
    "effect.blend_texture", "effect.fluid", "effect.glitter", "effect.god_rays.mode", "effect.light_shafts",
    "effect.tint", "effect.trail", "effect_masks", "layer_textures"] } }"""
const val RAIN_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
    "audio": { "attack": 0.05, "release": 0.3, "gain": "fixed" },
    "particles": { "rain": { "capacity": 60, "seed": 3, "emitters": [{ "shape": { "box": { "from": [4, 0], "to": [60, 4] } }, "rate": 60 }],
        "init": { "life": [2, 2], "speed": [0, 0], "size": [3, 3], "gradient": [[0, "#ff0000"], [1, "#0000ff"]] },
        "forces": [{ "gravity": [0, 200] }], "collide": [{ "bounds": "canvas", "bounce": 0.2 }],
        "render": { "order": "newest", "trail": { "length": 6, "width": 0.5 } } } },
    "layers": [{ "id": "rain", "type": "particles", "system": "rain",
        "opacity": { "audio": [0, 3], "of": 16, "bounds": [0.1, 0.9], "exponent": 2, "smooth": { "up": 0.05, "down": 0.5 }, "offset": 0.5 } }] }"""
const val RAIN_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.rain", "version": "1.0.0",
    "title": "Rain", "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
    "content": { "rating": "general" }, "photosensitive": false, "inputs": ["audio"],
    "requires": { "features": ["audio_ranges", "audio_response", "inputs.audio", "particles2", "shaping"] } }"""
const val FLASH_SCENE = """{ "schema": "wallpaper-rs.scene/1", "canvas": { "size": [64, 32] },
    "layers": [{ "id": "fill", "type": "solid", "color": "#ffffff" }] }"""
const val FLASH_MANIFEST = """{ "schema": "wallpaper-rs.wpak/2", "id": "org.example.flash", "version": "1.0.0",
    "default_locale": "en", "title": { "en": "Flash", "es": "Destello" }, "description": { "en": "Fast **flashes**.", "es": "Destellos **rápidos**." },
    "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "origin": { "kind": "original" },
    "content": { "rating": "mature", "descriptors": ["violence"] }, "photosensitive": true }"""
