# Packages

A `.wpak` package is a ZIP file with `manifest.json` at its root, the entry it names, and the
files that entry uses. `wprs-format` reads it strictly: stored or deflated entries only, no
encryption, no symbolic links, safe relative paths (no `..`, `\`, `:` or control characters,
at most 255 bytes), names unique without regard to case, at most 4096 files, 1 GiB per file and
2 GiB in total, a compression ratio of at most 100 for files above 1 MiB, and a manifest of at
most 1 MiB. Repositories (`repo.md`) and the server (`server.md`) distribute packages; scenes are
described in `scene.md`. An author signs the exact bytes of `manifest.json` into
`manifest.json.minisig`.

| `kind` | `entry` | Other files |
|--------|---------|-------------|
| `image` | png, jpg, jpeg, webp, avif, gif, bmp or ktx2 | images, txt |
| `video` | webm, mp4 or mkv | images, videos, txt |
| `scene` | `scene.json` | images, videos, json, wgsl, js, gltf, glb, bin, ogg, opus, flac, mp3, wav, ttf, otf, ttc, otc, woff2, cube, txt |
| `web` | an html or htm page, usually `index.html` | images (also svg, hdr and exr), videos, html, htm, js, mjs, css, json, wasm, ogg, opus, flac, mp3, wav, ttf, otf, woff, woff2, glb, gltf, bin, obj, mtl, fbx, basis, glsl, vert, frag, wgsl, csv, xml, txt |
| `preset` | `preset.json` | images, videos, json, txt |
| `assets` | a scene document such as `assets.json` | as `scene` |

Every kind may also carry `locale/<language>.json` (see "Languages"). No kind admits programs or
libraries (exe, dll, so, sh, bat, AppImage and the like are refused by their extension), and a
`.wasm` file must start with the WebAssembly signature. `image`, `video` and `scene` exist since the
first version; `web`, `preset` and `assets` need the second.

## Versions

`schema` is `wallpaper-rs.wpak/1` or `wallpaper-rs.wpak/2`, and every reader (daemon, library,
repositories, server, sync and the Android app) accepts both. A first-version package keeps
working unchanged: it has no rating, so clients show it as not rated and the `content:unrated`
filter finds it. Within `/2` only optional fields, enum values and feature names are added; the
meaning of what exists never changes, and a new major version is made only to remove or change
something. The first version keeps its rules: none of the fields below marked *2* may appear
in it, its texts are plain, its properties are written as the first version reads them (every
field it requires, such as the `default` of a text, the `accept` of a file and the `step` of a
slider, no field it does not know, and a whole `order` written without decimals) and its `file`
properties take images only, whatever `accept` lists.

Readers work in two steps. They first read a tolerant header (`schema` and `requires`): a newer
major version, a `requires.engine` that this version of wallpaper-rs does not meet, or a name in
`requires.features` that it does not know returns `Newer` (`this package needs a newer version of
wallpaper-rs: it uses …`) instead of an error about an unknown field. Only then is the manifest
read strictly, refusing unknown fields. `wprs-ctl pack upgrade DIR --rating general|mature
[--reason …] --photosensitive true|false [--derived-from AUTHOR]` turns a first-version folder into
the second version: it never guesses the rating, moves `preview` to `previews.thumbnail`, sets
`default_locale` when the texts are in several languages (`en` when all of them have English, or a
language they all share), renames the property ids with a `-` (`snow-speed` becomes `snow_speed`)
in the manifest and in the bindings and triggers of the scene, adds the inputs and features the
scene uses and removes a signature that no longer matches. `wprs-ctl scene upgrade` does the same in
the scene editor (`editor.md`).

In the desktop library a package that needs a newer version still shows its thumbnail
(`previews.thumbnail` or `preview`, read without the strict manifest) next to the reason. The
library remembers which version of wallpaper-rs read each package and reads every package again once
after an upgrade, so packages that needed a newer version start working and older ones get the
facets of the newer rules. Versions of wallpaper-rs released before the second version cannot open
its packages; repositories keep them readable for those versions by listing only first-version
releases where they look (`repo.md`).

## Manifest

| Field | Version | Rules |
|-------|---------|-------|
| `schema`, `id`, `version`, `kind`, `entry` | 1, 2 | `id` is a lowercase reverse-DNS name (`org.example.dusk`), `version` SemVer |
| `title` | 1, 2 | 1 to 128 characters on one line; in *2* also `{language: text}` or `"@key"` |
| `description` | 1, 2 | plain text in *1*; in *2* text, map or `"@key"` of at most 8000 characters, with the markup below |
| `changes` | 2 | notes of this version, at most 2000 characters, with the markup below |
| `default_locale`, `strings` | 2 | see "Languages" |
| `authors`, `license`, `tags` | 1, 2 | authors of 1 to 128 characters, an SPDX expression, at most 32 tags of 1 to 32 characters; in *2* authors and tags have no `<` or `>` |
| `platforms` | 2 | `[desktop, android]`, each once: the devices the package is made for, see "Platforms" |
| `content` | 2, required | `{rating: general \| mature, descriptors: [sexual, violence, gore, language]}`; `mature` names at least one reason, each once |
| `photosensitive` | 2, required | `true` when the package flashes or shows fast patterns that can trigger seizures |
| `origin` | 2, required | `{kind: original}` for a work made for wallpaper-rs, `{kind: derived, derived_from: [{author, title?, url?, license?}]}` for one remade by hand from 1 to 64 other works, crediting their authors (authors and titles without `<` or `>`), `{kind: imported}` for a conversion kept for personal use, or `{kind: provider, provider: {name, item_url, credit, terms_url?}}` for a picture or video downloaded from an online source; any kind may add `source_url`, see "Origin" |
| `credits` | 2 | at most 64 `{paths, license, attribution, url?, source?}`: who made which files (a path ending in `/` names a folder) under which SPDX license, and `source: {id, version}` when they were copied from an `assets` package; paths and attribution have no `<` or `>` |
| `links` | 2 | `{homepage, source, support, donate}`, each optional, `https://` and at most 512 characters |
| `preview` | 1 | an image of the package |
| `previews` | 2 | `{thumbnail, animated}`: png, jpg or webp of at most 1024 px a side and 1 MiB; gif or webp of at most 640 px, 15 s and 8 MiB; the file must be what its extension says |
| `requires` | 2 | `{engine: "<SemVer requirement>", features: [..], packages: [{id, version}], gpu: any \| compute}`, see "Features" and "Dependencies" |
| `inputs` | 2 | `[audio, pointer, clicks, media, network]`, see "Inputs" |
| `permissions` | 2, `web` and `scene` | `{network: {allow: [..]}, rgb}`: `network` for web pages only, see "Web pages"; `rgb` see "RGB lighting" |
| `web` | 2, `web` only | `{preload, background, storage_kib}`, see "Web pages" |
| `playback` | 2, `video` and `web` | how the content plays by default, see "Display and playback" |
| `display` | 2 | how a picture or video is placed and the accent it suggests, see "Display and playback" |
| `properties`, `presets` | 1, 2 | user properties (`scene.md`): in *1* at most 64 with ids `[a-z][a-z0-9_-]{0,62}` of the types `color`, `slider` with `step`, `bool`, `combo` of texts, `text`, `file`, `group` and `label`; in *2* at most 256 with ids `[A-Za-z][A-Za-z0-9_]{0,63}` that differ in more than case, combos of at most 64 options, files and folders that accept `image`, `video` or both, and every newer type and field; at most 32 presets in *1*, 64 in *2*, for scenes and videos |

```json
{
  "schema": "wallpaper-rs.wpak/2", "id": "org.example.dusk", "version": "1.3.0", "kind": "scene",
  "entry": "scene.json", "default_locale": "en", "title": { "en": "Dusk", "es": "Atardecer" },
  "description": "@description", "changes": "@changes",
  "strings": { "en": { "description": "Hills at **dusk**.", "changes": "- Warmer light", "speed": "Speed" },
               "es": { "description": "Colinas al **atardecer**.", "changes": "- Luz más cálida", "speed": "Velocidad" } },
  "authors": ["Example"], "license": "CC-BY-4.0",
  "credits": [{ "paths": ["assets/hills.webp"], "license": "CC0-1.0", "attribution": "Example" }],
  "origin": { "kind": "original" }, "content": { "rating": "general" }, "photosensitive": false,
  "previews": { "thumbnail": "preview.webp", "animated": "preview.gif" },
  "links": { "source": "https://example.org/dusk" },
  "requires": { "features": ["inputs.pointer"] }, "inputs": ["pointer"],
  "properties": { "speed": { "type": "slider", "label": "@speed", "min": 0, "max": 2, "step": 0.1, "default": 1 } }
}
```

## Display and playback

`display` takes `fit` (`fill`, `fit`, `stretch`, `center`, `tile` or `span`: the placement used when
a picture or video is applied without one), `background` (`#rrggbb`, the color around a picture or
video that does not cover the screen, used when it is applied without one), `span` (`per-output`,
or `all-outputs` to spread over every screen: it is then applied with `span` unless the user chooses
another placement) and `flip` (`none`, `horizontal` or `vertical`) for `image` and `video` packages,
and `zoom` (1 to 8) and `position` (two numbers of -1 to 1) for videos. Scenes may also set `span`,
only to `all-outputs`, since drawing on each screen needs no key: it asks to draw one canvas across every screen, each output showing its part with the
transform of its monitor; engines that cannot do that yet refuse such a scene with
`spanned_scenes`, and the phone, which has one screen, shows it there. `accent` suggests an accent
color for the desktop, `#rrggbb` or `{"prop": "<id>"}` of a color property, in any kind but
`assets`; no engine passes it to the desktop yet, so both platforms play such a package without
`accent` and say so.

`playback` takes `speed` (0.1 to 4), `start` and `end` (0 to 3600 s, `end` after `start`) and
`volume` (0 to 1) for videos, and `max_fps` (1 to 240, or `display` to follow the screen), `muted`
(it starts without sound) and `loop` (`false` plays it once and keeps the last frame) for videos
and web pages, whose page also takes `volume`. A setting the user makes always wins, and the frame
rate never exceeds what the user, the screen, the battery and the quality level allow.

### Video settings

A video package offers the same settings everywhere, which the user changes per screen or for every
screen like the properties of a scene (`GetSceneProperties`, `wprs-ctl prop`): `speed` (0.1 to 4
times, without changing the pitch of the sound), `start` and `end` (the part of the video that
loops, in seconds; an `end` of 0 plays to the end), `volume` (0 to 100 % of the general volume),
`zoom` (1 to 8) and `x` and `y` (-1 to 1, which part of the zoomed or cropped video shows). The
author gives their defaults in the second version:

```json
{ "kind": "video", "entry": "rain.webm",
  "playback": { "speed": 0.75, "start": 2.5, "end": 18, "volume": 0.4 },
  "display": { "fit": "fill", "zoom": 1.5, "position": [0, -0.5] },
  "presets": [{ "id": "calm", "label": "Calm", "values": { "speed": 0.5, "volume": 20 } }],
  "requires": { "features": ["video_properties"] } }
```

Presets of a video package set these ids.
Once the length of the video is known, `start` stays at least half a second before its end; a part
that plays nothing, such as a `start` past the end of the file, loops the whole video instead.
Speed and frame rate are independent: a faster video draws no more frames per second than the
screen, the battery or `playback.max_fps` allow, and changing the speed, the part or the volume
applies to the running video without starting it again.

## Rating and photosensitivity

The rating and the photosensitivity flag are declared by the author and never inferred. Clients
blur the thumbnails of packages rated `mature` until the user chooses to show them, from the
details of the item or with a click or a key on the blurred thumbnail of a screen in the app's
Settings page or the tray, and `library.hide_mature` leaves them out of searches and repository
listings. A photosensitive package shows a clear warning before it is applied, in the app,
`wprs-ctl` and the Android app, and the app can allow it from its details; the app's Settings page
and the tray warn on the screen that shows one. Until the user allows it the daemon refuses to
apply it (`Unconfirmed`), a profile leaves it out (the screen reports why) and playlists skip it.
Both decisions are kept once per package id (`Library1.Confirm`,
`wprs-ctl lib confirm mature|photosensitive ITEM`). The self-hosted server refuses packages rated
`mature`.

## Inputs

`inputs` declares what the package may ask the user for: the sound playing on the computer
(`audio`), the pointer (`pointer`), clicks (`clicks`), what the media player plays (`media`) and the
network (`network`, which only web pages use; other kinds that declare it get a warning). Declaring
is no grant: each input is still allowed by the user per package, when it is used. For scenes the
validator computes the inputs that the scene can use with any value of its properties and presets
(audio and pointer bindings, spectra, beat triggers, parallax, particles that follow the pointer
or burst on clicks, effect parameters such as the clicks of `water_ripple`) and refuses a manifest
that does not declare them, naming the missing one and the first place of `scene.json` that uses
it (`the scene uses audio at /layers/2/effects/0 of scene.json; add "audio"`).
Newer scenes are computed the same way: inverse kinematics and particle control points that follow the pointer, `enter:` and `leave:` triggers ask for `pointer`; `click:`
triggers for `clicks`; `media` values, cover assets and `media:` triggers for `media`. Scene scripts
declare in `scene.json` the parts of the script interface they use and the events they handle
(`scene.md`, "Scripts"), so their inputs are computed from that declaration rather than guessed from
their text. A second-version package receives only what it declares: the sound for `audio`, the
pointer for `pointer` or `clicks`, clicks for `clicks` and the media player for `media`, whatever its
scenes, scripts or page would read; in first-version scenes, scripts that declare nothing use the
inputs of what they import from the `wprs` module (`audio` and `input`). Web pages always declare their inputs, and their facets come from that list.

### RGB lighting

`permissions.rgb: true` says that the package may send colors to keyboards, mice and other RGB
lighting: a web page through `window.wprs.rgb.setImage`, a scene through an `adjustment` layer with
`rgb: true` (`scene.md`), which the validator requires to be declared. Like the inputs it is no
grant: the user allows it per package. Both name the `rgb` feature, which the desktop refuses until
it can send colors and the phone, which has no such lighting, leaves out.

## Features

`requires.features` lists, from a closed registry, what the package needs from the engine, so that
an older version or a weaker device says that it needs a newer one before reading anything else.
`wprs-ctl pack build` computes the list from the manifest and the scene and writes the missing
names into `manifest.json` before building (a signed folder must be signed again: `pack sign`
fills the list before signing); readers refuse a package that uses a feature it does not list,
and `wprs-ctl pack check` explains each one and warns about listed features the package does not
use.

Each feature also says what each platform does with it: the desktop and the Android app either run
it, play the package without it (and its page on the phone says so), or refuse the package with
`this package needs a newer version of wallpaper-rs: it uses the feature web (is a web page)`
before drawing anything, so that nothing is drawn wrongly or silently left out. The library still
lists a refused package with its thumbnail, title and that reason, and reads it again after an
update of wallpaper-rs. `wprs-ctl pack check` prints what each platform does.

| Feature | The package | Desktop | Android |
|---------|-------------|---------|---------|
| `inputs.audio`, `inputs.pointer`, `inputs.clicks`, `inputs.media`, `inputs.network` | declares or uses that input | runs | runs |
| `scripts` | runs scene scripts | runs | without |
| `sound` | plays sound layers | runs | without |
| `properties2` | has settings of the newer kinds or fields: sliders without `step` or with `decimals`, `editable` or `log`, combos of numbers or flags or with icons, `secret` texts, `button`, `divider`, `folder`, `shortcut`, collapsed groups, icons, fractional `order`, `visible_if` expressions, `enabled_if` and `default_asset`, or settings written in a way the first version cannot read (a field it does not know, even with its default value, a missing `default` of a text or `accept` of a file, or an `order` such as `2.0`) | runs | runs |
| `folder` | shows pictures from a folder the user chooses | runs | without |
| `folder2` | reads the subfolders of a chosen folder (`recursive`) or fewer of its files (`max_files`) | runs | without |
| `folder_lists` | lets its scripts or page list a chosen folder (`mode: all`) | refused | refused |
| `user_videos` | lets the user choose videos for its files or folders | runs | without |
| `shortcut` | has a `shortcut` property | refused | refused |
| `quality_settings` | has properties with `quality` values | refused | refused |
| `video_properties` | sets `playback` or `display` of a video | runs | without |
| `playback2` | sets `max_fps`, `muted` or `loop`, or `playback` of a web page | refused | refused |
| `display2` | sets `display` of a picture, or its `background` or `span` | runs | without |
| `flip` | sets `display.flip` to `horizontal` or `vertical` | refused | refused |
| `accent` | sets `display.accent` | without | without |
| `web` | is a web page (`kind: web`) | refused | refused |
| `preset` | sets the values of another package (`kind: preset`) | refused | refused |
| `assets` | holds resources for other packages (`kind: assets`) | refused | refused |
| `packages` | lists `requires.packages` | refused | refused |
| `gpu` | sets `requires.gpu` | without | without |
| `platforms` | sets `platforms` | runs | runs |
| `locale_files` | keeps texts in `locale/<language>.json` | runs | runs |
| `engine_strings` | names a text of the engine, `"@wprs.<key>"` | runs | runs |
| `newer_keys` | writes a key of the manifest or of `scene.json` that the released versions do not know, whatever its value (`null`, `[]` and `{}` included), while no other feature already tells them to update, so that they say that they need an update instead of calling the package broken | runs | runs |
| `origin2` | is `imported` or from a `provider`, or sets `origin.source_url` | runs | runs |
| `credits2` | credits files to the package they come from (`credits[].source`) | runs | runs |
| `ktx2` | uses KTX2 textures | refused | refused |
| `avif` | uses AVIF pictures | refused | refused |
| `spanned_scenes` | sets `display.span` of a scene to `all-outputs` | refused | without |
| `rgb` | sets `permissions.rgb` or sends colors to RGB lighting from a scene | refused | without |
| `effect.<id>` | uses a built-in effect that older versions do not have; `pack build` and `pack check` name the ones a package needs | runs once drawn, refused before | as on the desktop |
| `effect.<id>.<parameter>` | sets a parameter that an older version of a built-in effect does not have | runs once drawn, refused before | as on the desktop |

Effects and parameters that the format describes but the engine does not draw yet (the list is in
"Effects not drawn yet" of `scene.md`) are refused by every platform with these names, which stay
the same once they are drawn, so that older versions keep refusing them. `effect.tint`,
`effect.blend_texture` and `effect.light_shafts` run on both platforms.

The fields of `scene.json` that came after the first version name these features (`scene.md` says
what each field does). Each one is refused on a platform until its engine draws it, and the table
says which ones run today; the phone, which plays scenes without sound and without scripts, plays a
scene with `sound2` silently and leaves out `scripts2` and every `script_` feature with its scripts:

| Feature | The scene | Desktop | Android |
|---------|-----------|---------|---------|
| `blend_modes2` | uses a blend mode after `luminosity` (`linear_burn` … `negation`) | runs | runs |
| `layer_color` | sets `brightness` on a layer or `color` on a picture, video, particles or group layer | runs | runs |
| `parent` | sets `parent` or `inherit` on a layer | runs | runs |
| `tilt` | sets `transform.tilt`, `transform.z` or `canvas.perspective` | refused | refused |
| `mask_strength` | sets `strength` on a mask | runs | runs |
| `effect_masks` | masks an effect with a `layer` or a `shape` | runs | runs |
| `layer_textures` | feeds a layer into an effect texture with `{ "layer" }` | runs | runs |
| `video_textures` | uses a video asset as a mask, an effect texture or the picture of a light | refused | refused |
| `layer_playback` | sets `playback` on a picture or video layer | refused | refused |
| `texture_options` | sets `filter`, `wrap` or `mipmaps` on an asset | runs | runs |
| `animated_textures` | sets `frames`, `fps`, `durations`, `rects` or `animated` on an asset | runs | runs |
| `text_layout` | sets `valign`, `wrap`, `line_height`, `background` or `padding` on a text | runs | runs |
| `text_styles` | sets `letter_spacing`, `outline`, `shadow`, `blur` or `render: sdf` on a text | refused | refused |
| `fonts2` | uses ttc, otc or woff2 fonts, `face` or `fallback` | runs | runs |
| `sound2` | plays MP3, `sounds` lists, `mode`, `pause`, `stream` or `play_on` | runs | without |
| `camera` | sets `canvas.camera` | refused | refused |
| `parallax2` | sets `canvas.parallax` or a parallax with two different parts | runs | runs |
| `bloom` | sets `canvas.bloom` | refused | refused |
| `hdr` | sets `canvas.hdr` or `canvas.tonemap` | refused | refused |
| `keep_frame` | sets `canvas.clear` to `false` | refused | refused |
| `lights` | has `light` layers | refused | refused |
| `volumetric_lights` | sets `volumetric` on a light | refused | refused |
| `light_textures` | sets `texture` on a spot light | refused | refused |
| `lighting2d` | sets `lighting` on a layer | refused | refused |
| `ambient_light` | sets `canvas.ambient` or `canvas.sky` | refused | refused |
| `capture` | sets `size` or `redraw` on an adjustment layer | refused | refused |
| `timeline_events` | names `events` of a timeline or triggers on `event:<name>` | runs | runs |
| `timeline_frames` | gives the length of a timeline in `frames` at `fps` | runs | runs |
| `timeline_wrap` | sets `wrap` on a looping timeline | runs | runs |
| `fresh_seed` | sets `clock.seed` to `null`, a new seed every start | runs | runs |
| `tangents` | gives keyframes `{ "in", "out" }` handles | runs | runs |
| `tracks` | animates the parts of a position or a color on their own (`x`, `y`, `r`, `g`, `b`, `a`) | runs | runs |
| `relative_values` | adds a value to `base` | runs | runs |
| `clock_values` | follows `{ "clock": "day" }` and the like with a number | runs | runs |
| `clock_dates` | writes dates with a `date` text | runs | runs |
| `audio_ranges` | follows a range of bands (`audio: [lo, hi]`, `of`, `reduce`) | runs | runs |
| `shaping` | sets `bounds`, `exponent` or `smooth: { "up", "down" }` | runs | runs |
| `audio_response` | sets the scene `audio` block | runs | runs |
| `custom_effects` | brings effects of its own (`effects`) | runs | runs |
| `effect_options` | brings effects with `options` | runs | runs |
| `vertex_effects` | brings effects with a `vertex` stage | runs | runs |
| `custom_effects2` | brings effects with `compose`, `swap`, `min_fps` or `velocity`, or whose shader calls `cursor_last`, `grid_uv`, `grid_position` or `fs_copy` | runs | runs |
| `mesh` | has `mesh` layers, with their parts, depth, clips and animation layers | refused | refused |
| `ik` | sets `ik` chains on a mesh or model | refused | refused |
| `bone_physics` | sets `physics` on a mesh or model, or `canvas.wind` | refused | refused |
| `bone_rules` | sets `rules` on a mesh or model | refused | refused |
| `blend_shapes` | sets `shapes` or `expressions` on a mesh or model | refused | refused |
| `attach` | sets `attach` on a layer | refused | refused |
| `model3d` | has `model` layers or groups with `space: 3d` | refused | refused |
| `material_shaders` | shades a model with `fur`, `foliage`, `chroma` or a material shader of its own | refused | refused |
| `camera3d` | sets `camera3d` | refused | refused |
| `shadows` | sets `shadows` on a light, or `cast_shadows` or `receive_shadows` on a material | refused | refused |
| `particles2` | moves particles in world space, with control points, per-layer `instance` or `render`, scripts that use `particles`, emitter timing, newer emitter shapes, initializers and forces, operator windows, render `brightness`, `cycles`, `order` or a stretch range, `init.gradient`, a `mask` collider, or beyond the older limits (8 systems, 4 emitters, 8 forces, 4 colliders, 10 s of prewarm) | runs | runs |
| `particle_children` | sets `children` on a system | runs | runs |
| `particle_depth` | sets `perspective`, `init.depth` or `init.depth_speed` | refused | refused |
| `particle_lighting` | sets `render.lit`, `render.normal` or `render.refract` | refused | refused |
| `particle_layers` | emits from a `layer` or collides with one | refused | refused |
| `particle_fields` | uses `turbulent`, `offset_noise`, `remap`, `around`, `between`, `keep_distance` or `keep_between` | refused | refused |
| `particle_values` | binds an end of an `init` range or a field of an emitter shape | runs | runs |
| `particle_collisions` | collides with a `box`, a `segment` or the inside of a circle, or sets `stick` | refused | refused |
| `boids` | sets the `boids` force | refused | refused |
| `particle_ropes` | sets `render.rope` or trails longer than 16 steps or with `segments`, `subdivide`, `uv_scale`, `uv_scroll` or `textured` | refused | refused |
| `particle_looks` | sets `render.cutout`, `render.orient` or `render.angle` | refused | refused |
| `particle_shaders` | sets `render.shader` | refused | refused |
| `scripts2` | has more than 16 script modules, modules that declare newer events, parts or `params`, values with `fn` or `params`, or named effect uses | runs | without |
| `script_layers` | has `templates` or scripts that use `layers` | runs | without |
| `script_geometry` | has `pools` or scripts that use `geometry` | refused | without |
| `script_storage` | has scripts that use `storage` | refused | without |
| `script_sounds` | has scripts that use `sounds` | refused | without |
| `script_animations` | has scripts that use `animations` | refused | without |
| `script_camera` | has scripts that use `camera` | refused | without |
| `script_shortcuts` | has scripts that use `shortcuts` | refused | without |
| `script_hierarchy` | has scripts that use `hierarchy` | refused | without |
| `layer_events` | sets `solid: { "alpha" }`, `hit` bones, `click:`, `enter:` or `leave:` triggers or scripts that handle pointer events | refused | refused |
| `media_info` | follows `media` values, cover assets, `media:` triggers or scripts that use the media player | refused | refused |

## Profiles

Each platform is a profile: a column of the tables above that says, for every feature, whether that
platform runs it, plays the package without it or refuses the package. The registry in
`crates/wprs-format` (`features::all()`, `features::support`) holds both columns, and every reader
uses it: the desktop daemon and library, `wprs-ctl`, the app, repositories and the server (which
compute facets with it) and the Android app. A package is refused on a platform when any of
its features is refused there, with every such feature and its reason in one message; the features
a platform leaves out are listed on the package's page there; and the facets `desktop` and
`android` name the platforms that run a package whole. The package limits of the first part of this
page are the same on both platforms. The tests build one small package per feature
(`tests/fixtures/features.json`) and run it through `pack build`, the desktop daemon and the Android
library, so that a feature without its package, or a package that reaches an engine that cannot
draw it, fails them.

## Platforms

`platforms` lists the devices a package is made for, `desktop`, `android` or both, and can only
narrow where it plays: a package made for the phone only is refused on the desktop with `this
package is made for android only`, and the reverse. Facets name each platform that runs the
package fully (no feature refused or left out) and that its `platforms` allows, so that `facet:android`
finds what plays whole on the phone. Repositories copy `platforms` into the release.

## Dependencies

`requires.packages` lists at most 16 other packages, each `{id, version}` with a SemVer requirement
such as `^1.2`, that a package needs installed; it cannot name the package itself or one id twice.
`requires.gpu` is `any` (the default) or `compute` for packages that cannot draw without compute
shaders, so that the app can warn before applying them where they run in software; no engine
checks it yet, so both platforms play such a package without `gpu` and say so. Repositories copy
`requires` into the release.

## Origin

An `imported` package is a conversion for personal use: it installs and plays locally, but
`wprs-ctl repo add` and the server never publish it. A `provider` package names where its picture
or video comes from: `name` (`[a-z0-9-]`, at most 32), `item_url` and `terms_url` (`https://`),
and the `credit` that is always shown with it; it is published only under `CC0-1.0`, `CC-PDM-1.0`,
`CC-PDDC`, `CC-BY-2.0`, `CC-BY-3.0` or `CC-BY-4.0`, alone or in an `OR` of them. `source_url` is
the `https://` page of the original work. Clients show the origin, the provider's credit and the
packages that credited files come from with the details of the package.

## Web pages

A `web` package shows its entry page in an isolated web view, without network unless allowed:

```json
{ "kind": "web", "entry": "index.html", "inputs": ["audio", "pointer", "network"],
  "web": { "preload": "compat.js", "background": "#000000", "storage_kib": 512 },
  "permissions": { "network": { "allow": ["https://api.example.org"] } },
  "playback": { "max_fps": "display", "volume": 0.5 },
  "requires": { "features": ["inputs.audio", "inputs.network", "inputs.pointer", "playback2", "web"] } }
```

`web.preload` is a `.js` or `.mjs` file of the package run at the start of the page, before its own
scripts; `web.background` (`#rrggbb`) shows while the page loads; `web.storage_kib` (0 to 5120,
default 0) is the local storage the page may keep: none without it, and what it keeps stays on
the device, is never synced and goes away with the package or a *Clear data*. `permissions.network.allow`
lists at most 8 `https://` origins (`https://host` or `https://host:port`, lowercase, without a
path), the only places the page may reach once the user allows the network; a page that lists any
declares `network` in `inputs`, and only then. Pages receive their settings through `window.wprs`:

| Member | What the page gets |
|--------|--------------------|
| `properties`, `onProperties(cb)` | the values of every property when it loads, then only the changed ones; colors as `#rrggbb[aa]` and `{r, g, b, a}` from 0 to 1, combos with their value and `label` |
| `fps`, `onSettings(cb)` | the frame rate it may draw at, and `{fps, quality, locale}` when they change: the quality level (0 to 3) and the language picked among those of the package |
| `onPause(cb)` | `true` while it is hidden or paused, when animation frames stop too |
| `onAudio(cb)` | 64 bands per channel while `audio` is allowed, silence otherwise |
| `onMedia(cb)` | what the media player plays while `media` is allowed, `{enabled: false}` otherwise |
| `files(prop)`, `randomFile(prop)`, `onFiles(prop, cb)` | tokens `wpak-user://<prop>/…` for the files of a chosen folder, and the ones added and removed; never their paths |
| `onAction(cb)` | the id of a pressed `button` property |
| `openShortcut(prop)` | opens what the user chose for a `shortcut` property, only inside a real click and at most once a second |
| `rgb.setImage(bytes, w, h)` | sends RGBA pixels of at most 64 × 16 to the RGB lighting of the computer, at most 30 times a second, only with `permissions.rgb` and once the user allows it for the package |

Nothing reaches the page without the input it needs, and a page never sees paths, device names,
the battery or the list of fonts.

## Presets

A `preset` package carries values for another package, its base, and the pictures or videos those
values name; it is published, versioned and credited on its own:

```json
{ "kind": "preset", "entry": "preset.json",
  "requires": { "features": ["packages", "preset"], "packages": [{ "id": "org.example.dusk", "version": "^1.2" }] } }
```

```json
{ "base": { "id": "org.example.dusk", "version": "^1.2" },
  "values": { "tint": "#3d5aff", "speed": 0.4 }, "files": { "photo": "assets/photo.webp" } }
```

`preset.json` (at most 256 KiB) names the base, which `requires.packages` lists with the same
version requirement, at most 256 `values` and `files` by property id (an id is in one of them only),
and the files are pictures or videos of the package. A preset package has no properties, presets
or inputs of its own; values that the installed base does not know, of another type or out of range
are left out with a warning, like the values of a preset of the base. Repositories copy the id of
the base into the release (`base`), so that clients can list the presets of a package.

## Shared resources

An `assets` package offers parts of scenes for other authors: its entry is a scene document
(`scene.md`) whose layers, particle systems, scripts, effects (`effects`, see "Package effects" in
`scene.md`) and assets are the parts it offers, with their own files. It is never shown as a wallpaper. An author copies what they use into their own package
and credits it with `credits[].source`, so that the package works without the shared one and the
license of every file stays written.

## Languages

Texts are plain text, a map from language to text, or `"@key"`, a key of `strings`. `strings`
holds at most 64 languages, each with at most 2048 keys of `[A-Za-z0-9_.-]{1,64}` and 512 KiB in
all; languages are BCP 47 tags such as `en`, `es` or `pt-BR`. With maps or `strings`,
`default_locale` is required and every localized text has a value in it. Labels, help texts,
combo options and preset labels of properties take the same forms. A text is picked by comparing
tags without case: the user's languages in order (each as given, then its base language, `pt-BR`
then `pt`), then `default_locale`, then `en`, then the first one; `zh-chs`, `zh-cn` and `zh-sg`
read as `zh-Hans`, and `zh-cht`, `zh-tw`, `zh-hk` and `zh-mo` as `zh-Hant`. Control characters
(except line breaks in long texts) and byte order marks are refused.

The strings of a language can also live in `locale/<language>.json`, a map from key to text of at
most 512 KiB, which the package lists with the feature `locale_files`; a language is either in
`strings` or in its file, never both, and the limits above count both. Readers merge the files with
`strings` and keep them apart when they write the manifest back. Packages and source folders read these files
as texts only when they list `locale_files`; without it a `locale/*.json` file is an ordinary file
of the package, as it was before, so released packages and their folders that keep data of their
own there keep working and build as they did. The author chooses the feature: `pack build` never
adds it, and `pack build`, `pack check` and the editor note each `locale/<language>.json` that stays
an ordinary file.

A few common labels are texts of the engine, which every reader knows in its languages: `"@wprs.<key>"`
with `accent_color`, `background_color`, `brightness`, `color`, `density`, `opacity`, `quality`,
`size`, `speed` or `volume` (`engine_strings`). They need no `strings`, and keys of `strings` or of
the language files may not start with `wprs.`.

## Markup

`description` and `changes` use a small, safe markup: paragraphs separated by an empty line,
lists of lines starting with `- ` or `* `, `**bold**`, `*italic*` and links `[text](https://…)`.
A `*` or `**` opens before a word and closes after one on the same line; one without a partner is
refused, and `\*` writes an asterisk. HTML, images and links other than `https://` are refused.
Clients render it without loading anything remote and open a link only after a click and a
confirmation (through the OpenURI portal in the Flatpak); `wprs-ctl` prints it as plain text. Every
other text, and the description of a first-version package, is plain: clients show it as written,
and in the second version plain texts (titles, labels, help, options, preset labels) have no `<`
or `>`.

## Facets and tags

Repositories, the library and the server compute facets from the package itself, so that authors
cannot lie: the inputs the scene uses (`audio`, `pointer`, `clicks`), `sound`, `scripts`, `clock` (a
text shows the time), `customizable` (it has properties to change), `photosensitive`, `localized`
(more than one language), `animated` (scenes, videos and web pages), `desktop` and `android` (it
runs fully there, see "Platforms"), the orientation (`landscape`, `portrait`, `square`),
`ultrawide` from a 2.2 ratio and the size (`720p`, `1080p`, `1440p`, `4k`, `5k`, `8k`) of scenes
and images, and `hdr` when a video of the package (its entry, or a video asset of its scene) writes
the PQ or HLG transfer in its colour description (the `colr` box of MP4 or the colour elements of
Matroska and WebM). Such a video plays with standard colors, so it may look washed out until the
engine maps HDR tones; `wprs-ctl pack check` warns about it. Readers look for it only when they
record facets (adding to a repository, indexing the library, the server and `pack check`), never
when they open a package to show it. The inputs of a web page come from its manifest as declared;
for the other kinds every input, `media` included, comes from what the package uses, and an
`audio`, `pointer`, `clicks`, `media` or `network` input that nothing uses gets a warning and no
facet. Since only web pages use `network`, released second-version packages that declare it
elsewhere get that warning and keep working. Searches filter by
them (`facet:audio`, `facet:4k`) and by rating (`content:general`, `content:mature`,
`content:unrated`). Tags stay free; these lowercase slugs are recommended for genres: `abstract`,
`anime`, `animals`, `cars`, `cityscape`, `fantasy`, `games`, `minimal`, `mountains`, `music`,
`nature`, `pixel-art`, `retro`, `sci-fi`, `space`, `water`, `winter`.
