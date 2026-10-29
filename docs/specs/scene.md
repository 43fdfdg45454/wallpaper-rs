# Scene format

A scene package is a `.wpak` package (see `package.md` for packages and their manifest, `repo.md`
for repositories) whose manifest has `"kind": "scene"` and `"entry": "scene.json"`. The scene
lists its assets by package path; every path must be in the package. `wprs-ctl pack build` and
every reader check the scene: unknown fields, broken references and out-of-range values are refused
with the JSON path of the field, such as `/layers/2/effects/0/params/radius is 300, expected
0..=128`. The model lives in
`crates/wprs-scene`; `wprs-ctl scene bench <file.wpak>` renders a package offscreen and reports
what each frame costs.

```json
{
  "schema": "wallpaper-rs.scene/1",
  "canvas": { "size": [1920, 1080], "fit": "fill", "background": "#0b1020", "blend_space": "linear" },
  "clock": { "max_fps": 60, "poster_time": 3.0, "seed": 42 },
  "assets": { "sky": { "path": "assets/sky.png" }, "mono": { "path": "assets/mono.ttf" } },
  "layers": [
    { "id": "sky", "type": "image", "image": { "prop": "style", "map": { "night": "sky" } },
      "effects": [{ "effect": "waves", "mask": "lake", "params": { "amplitude": 4 } }] },
    { "id": "clock", "type": "text", "font": "mono", "size": 96, "text": { "clock": "HH:mm" },
      "anchor": "bottom_right", "transform": { "position": [-64, -64] },
      "opacity": { "timeline": "intro", "keys": [[0, 0, "out_cubic"], [1.5, 1]] } }
  ],
  "timelines": [{ "id": "intro", "duration": 1.5, "loop": "once" }]
}
```

## Canvas, clock and assets

- `canvas.size` is the reference size in pixels (`1..=16384`); coordinates have their origin at
  the top left and angles are clockwise degrees. `fit` places the canvas on each screen (`fill`,
  `fit`, `stretch`, `center`). `background` is `#rrggbb[aa]` or `[r, g, b, a]` from 0 to 1.
  `blend_space` is `linear` (default) or `srgb`, which mixes encoded values as most editors do.
- `clock.max_fps` caps the frame rate (`1..=240`, or `"display"`, also `0`, to follow the refresh
  rate of each output for motion that gains from it; default 60), `poster_time` is the instant
  shown where motion is not wanted or where the device cannot run the scene even at its lowest
  quality level, and the one drawn as the library thumbnail of a package without a preview picture (with clocks and
dates at the local time it is drawn), `seed` feeds procedural effects; `"seed": null` picks a new seed each time the
  scene starts, so its random patterns differ every time (`fresh_seed`, which every platform runs).
- `assets` maps at most 256 ids (`[a-z0-9_]{1,32}`) to `{ "path", "linear", "atlas" }`. Images
  (PNG, JPEG, WebP, GIF, BMP), fonts (TTF, OTF), videos (WebM, MP4, MKV, stored uncompressed),
  sounds (Ogg Vorbis, Opus, FLAC or WAV, see "Sound") and color lookup tables (`.cube`,
  `LUT_3D_SIZE` 2 to 65, domain 0 to 1, at most 16 MiB each and 64 MiB of table data per scene; a
  file named by several ids is read once) are used.
  `linear` marks data images such as depth or flow maps, which are read without color decoding.
  `scene.json` is at most 8 MiB and the images of a scene at most 512 MiB once decoded.

The fields below came after the first version of the scene format. Each one names a feature of
`requires.features` (the table in `package.md`), which `wprs-ctl pack build` fills in; an engine
that cannot draw a feature yet refuses the whole package with `this package needs a newer version
of wallpaper-rs: it uses the feature …` instead of drawing it wrongly or leaving it out, and keeps
its thumbnail and details in the library. Scenes without them read and draw as before.

| `canvas` field | Meaning | Feature |
|----------------|---------|---------|
| `parallax` | `{ "amount" (0 to 4, default 1), "delay" (0 to 2 s, default 0.15), "pointer" (0 to 1, default 1), "gyro" }`: how far and how softly layers with a `parallax` follow the pointer, how much the pointer counts and whether the tilt of the phone moves them too (on Android, through the same motion sensors as picture parallax, only while the scene is shown); `pointer: 0` needs no pointer; a layer moves by its `parallax` times the canvas size times `amount` times the offset of the pointer from the middle (−0.5 to 0.5) times `pointer`, plus half the tilt; where the system asks for reduced motion, layers stay in place and neither the pointer nor the tilt moves them | `parallax2` |
| `camera` | `{ "center", "zoom" (0.1 to 10), "rotation", "shake" }` moves, zooms and turns the whole scene as one transform without extra passes; `shake` is `{ "enabled", "amplitude" (0 to 1000 px), "roughness" (0 to 1), "speed" (0 to 60 Hz) }`, turned off where motion is reduced and at the lowest quality level; all of them may be bound | `camera` |
| `bloom` | `{ "threshold" (0 to 4), "strength" (0 to 8), "radius" (0 to 128 px), "tint", "hdr": { "threshold" (0 to 16), "strength" (0 to 8) } }`, a glow of the bright parts drawn after everything else; `hdr` is used only at the highest quality levels on mains power | `bloom` |
| `hdr`, `tonemap` | `hdr: true` keeps colors brighter than white in 16-bit intermediate images, and `tonemap` (`none`, `aces` or `agx`, only with `hdr`) maps them to the screen | `hdr` |
| `clear` | `false` draws every frame over the previous one, which leaves trails; it needs a full redraw of every frame, so it costs more | `keep_frame` |
| `perspective` | distance of the viewer in pixels (100 to 100000) for layers tilted with `transform.tilt` and moved with `transform.z` | `tilt` |
| `ambient`, `sky` | colors of the light around the scene and from above, for lit layers | `ambient_light` |
| `wind` | `[x, y]` pixels per second, may be bound: the wind that moves bones with `physics` and models with the `foliage` material | `bone_physics` |

The scene may also set `audio: { "attack" (0.001 to 2 s), "release" (0.01 to 10 s), "gain": "auto" |
"fixed" }`, how fast audio values rise and fall and whether they follow the system volume
(`fixed`) or adjust themselves (`auto`, the default) (`audio_response`).

| Asset field | Meaning | Feature |
|-------------|---------|---------|
| `filter` | `linear` or `nearest` (sharp pixels) sampling of a picture | `texture_options` |
| `wrap` | `clamp`, `repeat` or `mirror` outside the picture: with `repeat` or `mirror`, an `image` layer whose `size` is larger than the picture (or than one frame of a sheet) fills it with copies at the picture's own size instead of stretching it | `texture_options` |
| `mipmaps` | `true` or `false` overrides whether smaller copies are made for pictures drawn smaller than they are; by default they are made once, the first time a picture is drawn at less than three quarters of its size | `texture_options` |
| `atlas`, `rects` | a sprite sheet as `[rows, columns]` of equal cells (as for particles) or as at most 256 `[x, y, width, height]` rectangles in pixels, not both | `animated_textures` with the fields below |
| `frames` | how many cells play (1 to the number of cells, at most 256) | `animated_textures` |
| `fps`, `durations` | 0.1 to 120 frames a second, or one duration per frame (0.001 to 60 s), not both | `animated_textures` |
| `animated` | `true` plays the frames of an animated GIF or WebP with their own durations (at most 256 frames and 64 MiB once decoded; a frame shorter than 20 ms lasts 100 ms, as in browsers) | `animated_textures` |
| `face` | which font of a `.ttc` or `.otc` collection (0 to 255) | `fonts2` |
| `media` | `current` or `previous`: the cover of what the media player plays now or played before, usable wherever a picture is; `path` is then the picture shown while nothing plays | `media_info` |

An `image` layer that shows a sheet plays its frames on the scene clock, so that every layer showing
the same sheet at the same rate shows the same frame; its `playback` sets the rate, the range, whether
it loops and the frame (`layer_playback`, which no platform runs yet, so a package that uses it is
refused). A sheet without `fps` or `durations` shows its first frame. The scene asks for a frame only when the next frame of a shown
sheet is due, never while the layer is hidden, and the lowest quality level shows the first frame of
the range without moving.

Fonts may also be TrueType or OpenType collections (`.ttc`, `.otc`) and compressed `.woff2` files
(`fonts2`). Videos may be used as masks, effect textures and the picture of a spot light
(`video_textures`); sounds may also be MP3 (see "Sound"). KTX2 textures are accepted by the package
reader but refused with `ktx2` until the engine loads them. Meshes and models are binary glTF 2.0
files (`.glb`, at most 64 MiB each; FBX or OBJ convert to it with common 3D tools): readers read
only the header and the JSON part of each file once (at most 4 MiB, and 16 MiB for all the models
of a scene), refuse files that load anything from outside themselves or require an extension other
than `KHR_materials_emissive_strength`, `KHR_materials_unlit`, `KHR_mesh_quantization` and
`KHR_texture_transform`, skins of more than 256 bones, more than 64 blend shapes per primitive,
more than 65 536 nodes or 4096 meshes, materials, animations or skins, more than 262 144 list
entries for all the models of a scene and more than 2 000 000 triangles, and read the names of its nodes (attachment points), skin joints
(bones), meshes (parts), materials, blend shapes (`extras.targetNames`, or `shape_<n>`) and
animations with their length, against which every name the scene gives is checked. Unnamed
elements are called `node_<n>`, `mesh_<n>`, `material_<n>` and `animation_<n>`.

## Layers

Every layer has a unique `id` and a `type`, and may set `transform` (`position`, `scale`,
`rotation`, `skew`, `origin`), `anchor` (`top_left` … `bottom_right`: the position is then
relative to that point of the visible part of the canvas), `opacity`, `visible`, `blend`, `mask`,
`effects` (at most 16), `parallax` (moves the layer with the pointer), `solid` and `cache`.
Layers without `anchor` are centered on the canvas by default and `origin` is a fraction of the
layer's own box (default `[0.5, 0.5]`, or the anchor point).

| `type` | Fields |
|--------|--------|
| `group` | `layers`, `clip` (later children only show where the first one is) |
| `image` | `image` (asset id), `size` |
| `video` | `video` (asset id, or a value such as `{ "prop", "map" }`), `size`; plays in a loop |
| `text` | `text`, `font` (asset id or system family), `size`, `color`, `align` |
| `shape` | `shape` (`rect` with `radius`, `ellipse`, `ring`, `polygon`, `star`, `line`), `fill`, `gradient` (`radial`, `from`, `to`, 2 to 4 `stops`), `stroke` (`color`, `width`) |
| `spectrum` | `size`, `style` (`bars`, `circle`, `wave`), `bands` (16, 32 or 64), `channel`, `color`, `gap` |
| `solid` | `color`, `size` (the canvas by default) |
| `particles` | `system`, a key of `particles` (see below) |
| `sound` | `sound` (asset id), `loop`, `play`, `volume`, `pan`, `fade_in`, `fade_out` (see "Sound"); no visual field |
| `light` | see "Lights"; draws nothing itself |
| `adjustment` | only `effects`, applied to everything below it; `size`, `redraw` and `rgb`, see below |
| `mesh` | a picture bent by a skeleton, see "Meshes" |
| `model` | a 3D model, see "Models and 3D" |

`blend` is `normal`, `add`, `multiply`, `screen`, `lighten`, `darken`, `overlay`, `soft_light`,
`hard_light`, `difference`, `exclusion`, `color_dodge`, `color_burn`, `hue`, `saturation`,
`color` or `luminosity`. Composition uses premultiplied alpha. `mask` is an asset id or
`{ "texture" | "layer" | "shape", "channel", "invert", "feather", "strength" }`: textures and layers mask by
luminance or alpha (`r`, `g`, `b`, `a`, `luminance`), shapes by their outline. A layer used as a
mask (or a group holding one) may set `visible: false`: it is then not drawn but still masks. At
most 256 layers nested at most 8 levels; masks may not refer to each other in a cycle.

`blend` also takes these modes, computed per channel on the backdrop B and the layer S, both from
0 to 1, before the usual premultiplied composition (`blend_modes2`): `linear_burn` max(0, B + S − 1),
`linear_dodge` min(1, B + S), `darker_color` and `lighter_color` (the whole color of B or S with the
lower or higher luminance), `vivid_light` (color burn with 2S below one half, color dodge with
2S − 1 above), `linear_light` clamp(B + 2S − 1), `pin_light` (min(B, 2S) below one half, max(B,
2S − 1) above), `hard_mix` (1 where B + S ≥ 1, else 0), `subtract` max(0, B − S), `divide` (1 where S
is 0, else min(1, B / S)), `reflect` (1 where S is 1, else min(1, B² / (1 − S))), `glow` (reflect with
B and S swapped), `phoenix` min(B, S) − max(B, S) + 1, `average` (B + S) / 2 and `negation`
1 − |1 − B − S|.

| Layer field | Meaning | Feature |
|-------------|---------|---------|
| `brightness` | 0 to 16 (default 1), multiplies the color of the layer; above 1 it only shows with `canvas.hdr` | `layer_color` |
| `color` | on `image`, `video`, `particles` and `group` layers, a color that multiplies the layer (white by default); a group applies it to its children together | `layer_color` |
| `parent` | the id of another drawn layer whose transform moves, turns and scales this one without changing the drawing order; the position of the layer is then relative to the position of its parent, in its turned and scaled frame (at the parent's position by default), instead of to its group; chains go at most 8 deep and never back to themselves | `parent` |
| `inherit` | what follows the parent: `transform`, `visible` and `opacity` (default `transform` and `visible`), only with `parent` | `parent` |
| `transform.tilt` | `[x, y]` degrees (−89 to 89) of tilt in depth, seen with `canvas.perspective` | `tilt` |
| `transform.z` | pixels towards the viewer (−100000 to 100000) | `tilt` |
| `parallax` | a number or `[x, y]` (−10 to 10 each), with a different depth per axis | `parallax2` when the two differ |
| `lighting` | on `image`, `video`, `text` and `particles` layers, `{ "normal", "roughness", "reflection", "emissive", "strength", "env" }`: a `linear` normal map, roughness and reflection from 0 to 1, light of its own from 0 to 16, how much the lights count from 0 to 1 and a picture to reflect, how the layer takes the light of `light` layers | `lighting2d` |
| `playback` | on `image` layers showing moving pictures or videos and on `video` layers, `{ "rate" (0 to 16), "playing", "loop", "start", "end" (seconds), "frame" }`; `frame` picks a frame of a sprite sheet and may be bound to keyframes, the pointer, the audio or a script | `layer_playback` |
| mask `strength` | 0 to 1 (default 1), weakens the mask: m′ = mix(1, m, strength) | `mask_strength` |
| adjustment `size` | `[width, height]` in pixels (at most twice the canvas): only that rectangle around the layer position is captured, and `redraw: true` draws what was captured again with the transform of the layer, as a magnifier or a mirror | `capture` |
| adjustment `rgb` | `true` sends what the layer captures, shrunk to at most 64 × 16, to the RGB lighting of the computer, at most 30 times a second; the manifest must set `permissions.rgb` (`package.md`) | `rgb` |
| `attach` | `{ "layer", "bone" \| "point", "offset": [x, y], "inherit" }`: follows a bone or a node of a `mesh` or `model` layer instead of a `parent`; `inherit` lists `position`, `rotation` and `scale` (all by default); a layer may not follow itself, through other attachments either | `attach` |
| `solid` | `true` or `{ "alpha" (0 to 1) }`: the layer takes the pointer for `click:`, `enter:` and `leave:` triggers and script pointer events, over its box or, with `alpha`, only where it is at least that opaque; layers are tested from the top down | `layer_events` with `alpha` |
| `depth_test` | inside a group with `space: 3d`, a flat layer hides behind the models in front of it instead of covering them | `model3d` |

### Text

A `text` layer also takes these fields:

| Field | Meaning | Feature |
|-------|---------|---------|
| `fallback` | at most 8 more fonts (asset ids or system families) tried in order for characters `font` lacks; only `sans-serif`, `serif` and `monospace` are sure to exist, and content never sees the list of installed fonts | `fonts2` |
| `valign` | `top`, `center` (default) or `bottom`: which edge of the text stays at the layer position while lines come and go (the vertical part of the default `origin`) | `text_layout` |
| `wrap` | `{ "width" (1 to 16384 px), "lines" (1 to 1000), "ellipsis" }` breaks lines at that width (after spaces and hyphens, between the letters of Chinese, Japanese and Korean, and inside a word longer than the line), keeps at most `lines` of them and ends a cut text with an ellipsis; `align` then works within that width | `text_layout` |
| `line_height` | 0.5 to 4 times the size | `text_layout` |
| `background` | `{ "color", "padding": [x, y], "radius" }`, a box behind the text | `text_layout` |
| `padding` | 0 to 1024 px of margin around the text for its effects | `text_layout` |
| `letter_spacing` | −100 to 1000 px between letters | `text_styles` |
| `outline` | `{ "color", "width" (0 to 64 px) }` | `text_styles` |
| `shadow` | `{ "color", "offset": [x, y], "blur" (0 to 64 px) }` | `text_styles` |
| `blur` | 0 to 64 px | `text_styles` |
| `render` | `auto` (default), `raster` or `sdf`, which draws from a distance field so that the size can change without drawing the letters again | `text_styles` with `sdf` |

Outline, shadow, blur and letter spacing may be bound like other values.

A text is laid out again only when its text, size on screen or layout changes; its color, the color of
its background and its position never lay it out again. Each character comes from the first font of
`font` and `fallback` that has it, then from the system `sans-serif`; letters are placed one after
another with the kerning of their font, without joining or reordering them, so scripts that need
shaping (Arabic, Indic scripts) or right-to-left text do not show correctly yet. A font file is at most
32 MiB, also once a `.woff2` is unpacked; a font that cannot be read is replaced by the system one.

### Lights

A `light` layer lights the layers that have `lighting` (`lights`); it draws nothing, so it takes only
`id`, `type`, `transform` (its position, its rotation for the direction of spot and directional
lights, and `tilt`), `anchor`, `visible`, `parent`, `inherit` and these fields. A scene has at most
12 lights.

| Field | Meaning |
|-------|---------|
| `light` | `point` (default), `spot`, `tube` or `directional` |
| `color`, `intensity` | the color (white by default) and 0 to 100 (default 1) |
| `radius` | 1 to 100000 px (default 512), where the light ends |
| `z` | height above the canvas, −100000 to 100000 px (default 100) |
| `cone` | spot lights only: `[inner, outer]` angles from 0 to 180 degrees |
| `length` | tube lights only: 1 to 100000 px |
| `volumetric` | point and spot lights only: `{ "density" (0 to 1), "samples" (4 to 64) }` lights the air (`volumetric_lights`) |
| `texture` | spot lights only: a picture or video the light projects (`light_textures`) |
| `shadows` | point, spot and directional lights: `true` casts shadows of models and of materials that `receive_shadows`; at most 2 lights per scene (`shadows`) |

A light may also follow a bone with `attach`.

### Meshes

A `mesh` layer (`mesh`) draws a binary glTF of flat parts (positions with an optional depth, UVs and,
for bending, `JOINTS_0` and `WEIGHTS_0` with a skin) bent by its skeleton, such as a character cut
into limbs. A scene has at most 16 `mesh` and `model` layers.

| Field | Meaning |
|-------|---------|
| `mesh` | the `.glb` asset |
| `texture` | a picture or video asset (or bound value) that replaces the textures of the file |
| `size`, `color` | the box in pixels the mesh fills (the canvas by default) and a color that multiplies it |
| `parts` | per part (mesh name of the file): `order` (−1000 to 1000, may follow keyframes with `step`), `clip_to` (another part it only shows over) and `visible` |
| `depth` | `{ "scale" (0 to 10), "cull" }`: how far the depth of the vertices pushes them with `canvas.perspective` or the 3D camera, and whether parts seen from behind are hidden |
| `clips` | at most 64 `{ "id", "animation", "from", "to", "loop", "events" }`: a stretch of an animation of the file (the first one without `animation`) in seconds within its length, played `repeat` (default), `pingpong` or `once`, with named `events` like timelines |
| `animations` | at most 16 animation layers `{ "clip", "weight" (0 to 1), "rate" (0 to 16), "additive", "blend_in", "blend_out" (0 to 10 s), "enabled", "bones" }`, mixed in order: replacing layers first, then additive ones; `bones` limits a layer to those bones; bones without a track keep their rest pose |
| `ik` | at most 16 chains `{ "chain" (2 to 8 bones from the root), "target", "limits": { "<bone>": [min, max] }, "weight", "bend": "clockwise" \| "counterclockwise" }`; `target` is a point (which may be bound), `"cursor"`, `"bone:<name>"` or `"layer:<id>"`; solved after the animations (`ik`) |
| `physics` | at most 64 `{ "bone", "mode": "spring" \| "rigid" \| "rope", "stiffness", "damping", "inertia", "wind" (0 to 1), "gravity" (−10 to 10), "rotation": [min, max] degrees, "translation": [min, max] px }` simulated at a fixed 60 Hz so that it does not depend on the frame rate, after the inverse kinematics (`bone_physics`) |
| `rules` | at most 64 `{ "bone", "parent", "weight" }`: moves a bone over to another parent as `weight` goes from 0 to 1, keeping its pose, as a sword passing from a rock to a hand (`bone_rules`) |
| `shapes`, `expressions` | weights from 0 to 1 of the blend shapes of the file, and at most 64 expressions `{ "<name>": { "<shape>": weight (−1 to 1) } }` whose names `shapes` may also set; at most 8 shapes act at once, the heaviest (`blend_shapes`) |
| `hit` | bones whose area takes the pointer for scripts, which then learn the bone that was touched (`layer_events`) |

```json
{ "id": "fox", "type": "mesh", "mesh": "fox", "size": [600, 900],
  "clips": [{ "id": "idle", "animation": "idle", "events": [{ "t": 1.2, "name": "blink" }] }],
  "animations": [{ "clip": "idle" }],
  "ik": [{ "chain": ["neck", "head"], "target": "cursor", "limits": { "head": [-30, 30] } }],
  "physics": [{ "bone": "tail", "mode": "rope", "wind": 0.6 }],
  "shapes": { "smile": { "prop": "mood" } } }
```

### Models and 3D

A `model` layer (`model3d`) draws a binary glTF model with metallic-roughness materials. Alone it
renders into its own box (`size`, the canvas by default) with depth, and then composes like any
layer; a `group` with `space: "3d"` renders its models, lights and particles in one pass with a
shared depth, into which flat layers with `depth_test` fit. Models also take `clips`,
`animations`, `ik`, `physics`, `rules`, `shapes`, `expressions` and `hit` as meshes do, and:

| Field | Meaning |
|-------|---------|
| `model` | the `.glb` asset |
| `position`, `rotation`, `scale` | three numbers each (or one for all three), in the units of the model and degrees, which may be bound |
| `root_motion` | `true` lets the animation of the root move the layer; needs `clips` |
| `env` | a picture reflected by the materials |
| `materials` | per material name of the file, at most 64 of `{ "shader", "color", "emissive", "lit", "double_sided", "cast_shadows", "receive_shadows", "cull": "back" \| "front" \| "none", "depth_test", "depth_write", "blend": "opaque" \| "mask" \| "blend" \| "add", "params" }` |

`shader` is `pbr` (default, `params` `metallic`, `roughness`, `occlusion` 0 to 1 and `normal_scale`
0 to 10), `unlit`, `fur` (`layers` 1 to 32, `length` 0 to 100, `density` 0 to 1), `foliage`
(`strength` and `speed` 0 to 10, swaying with `canvas.wind`), `chroma` (`key` color, `threshold` and
`softness` 0 to 1) or the id of a package effect with `stage: "material"` (`material_shaders` for all
but `pbr` and `unlit`). Shadows of lights and materials name `shadows`.

`camera3d` at the root of the scene (`camera3d`) is the camera of models, 3D groups and tilted
layers: `fov` (1 to 179 degrees, may be bound), `near` and `far` planes, `eye`, `center` (both may be
bound) and `up`; `paths` (at most 16 `{ "id", "keys", "ease", "loop" }`, each with 2 to 256 keys `{
"t", "eye", "center", "up", "fov" }` in increasing time) play one after another or at random
(`order`), joined by a `fade` `{ "to": "black" | "cross", "duration" (0 to 10 s) }`.
## Values

Every number, color, flag, text and image of a layer may be a literal or an object with exactly
one source:

| Source | Example |
|--------|---------|
| User property | `{ "prop": "style", "map": { "night": "sky" } }` |
| Keyframes | `{ "keys": [[0, 0], [2, 1, "in_out_sine"]], "timeline": "intro", "loop": "repeat" }` |
| Audio | `{ "audio": "bass" }`, `{ "audio": 3, "channel": "left" }` |
| Pointer | `{ "cursor": "x" }` (`x`, `y`, `inside`, `speed`) |
| Clock | `{ "clock": "HH:mm:ss a" }` (text only: `HH`, `H`, `hh`, `h`, `mm`, `m`, `ss`, `s`, `a`) |
| Script | `{ "script": "scripts/spin.js" }`; with `"fn": "pulse"` and `"params": { "speed": 2 }` a module serves many values (`scripts2`, see "Scripts") |
| Media player | `{ "media": "title" }` (`media_info`, see below) |

Non-literal values accept `scale`, `offset`, `clamp: [min, max]`, `invert` and `smooth` (seconds).
Easings are `linear`, `step`, `hold`, `in_`, `out_` or `in_out_` followed by `sine`, `quad`,
`cubic`, `quart`, `quint`, `expo`, `circ`, `back`, `elastic` or `bounce`, or a cubic Bézier
`[x1, y1, x2, y2]`. Colors interpolate in the OKLab space, lightness, chroma and hue apart, the hue
along the shorter way round, so that red to green passes through orange and not through a dark
olive, and a grey takes the hue of the other end.

`timelines` (at most 64) have `id`, `duration`, `loop` (`once`, `repeat`, `pingpong`), `speed`
(−100 to 100; below zero the timeline runs backwards from its start without `delay`, so a loop
plays from its end and one that plays `once` ends at once), `delay`, `autoplay` and `trigger` (`start`, `beat` or `prop:<id>`, which also fires when the
`button` setting `id` is pressed). Instead of `duration` (seconds), a timeline may count `frames`
at `fps` (1 to 240) frames a second, both together and never with `duration`; keys stay in
seconds (`timeline_frames`, which every platform runs). `wrap: true`, only on a timeline that loops
with `repeat`, blends the values from their last key back to their first one before the loop
starts again: the stretch from the last key to the first key of the next pass is one more segment
with the easing of the last key, so a value with keys at 1 s and 3 s on a 4 s timeline goes from
the last value back to the first between 3 s and 5 s (`timeline_wrap`, which every platform runs). The scene
clock only runs while the output is shown and active, keeps real time while a still scene waits
(up to ten minutes between frames, in steps of at most 0.1 s), and never catches up after a pause.

Newer values add these forms, each with its feature:

| Form | Example | Feature |
|------|---------|---------|
| Keyframe handles | `[2, 1, { "in": [0.5, 0], "out": [0.5, 0.2] }]`: Bézier handles in time and value around a key of a number, the time of each within the neighbouring segment; `out: [dt, dv]` is the point (t + dt, v + dv) and `in: [dt, dv]` the point (t − dt, v − dv), so equal handles make a smooth key and `dv: 0` a flat one; a side without a handle points a third of the way to the next key | `tangents` |
| Parts on their own | `{ "x": { "keys": [[0, 0], [2, 100]] }, "y": { "cursor": "y", "scale": 100 } }` for a position, `r`, `g`, `b`, `a` for a color; each part is a value of its own and a missing part is that of `base`, or 0 (alpha 1) | `tracks` |
| Relative | `{ "keys": [[0, 0], [1, 30]], "base": 120 }` adds the animation to `base` (numbers, positions and colors, part by part) | `relative_values` |
| Time as a number | `{ "clock": "day" }` from 0 at midnight to almost 1 before the next; also `hour` (0 to 23), `minute` and `second` (0 to 59), `weekday` (1 for Monday to 7 for Sunday) and `month` (1 to 12), all in local time; a value that follows `day` changes at most once a minute, or every `clock.step` seconds (1 to 3600), and the others when they change | `clock_values` |
| Dates | `{ "date": "EEEE d 'de' MMMM yyyy" }` (text only) with `yyyy`, `yy`, `MMMM`, `MMM`, `MM`, `M`, `dd`, `d`, `EEEE`, `EEE`, `D` (day of the year) and `w` (ISO week) besides the tokens of a clock, names in the language of the package (English or Spanish, English for the others); text between single quotes stays as written and `''` writes a quote; a text without hours, minutes or seconds changes at midnight. A `clock` text keeps only the tokens of the time, and every other letter stays as written | `clock_dates` |
| Frequency ranges | `{ "audio": [0, 3], "of": 16, "reduce": "max" }` follows bands 0 to 3 of 16 (`of` is 16, 32 or 64, default 64, also for a single band) and takes their `mean` (default) or `max` | `audio_ranges` |
| Shaping | `bounds: [a, b]` maps a value to (x − a) / (b − a) clamped to 0 to 1, `exponent` (0.1 to 10) raises it, both before `scale` and `offset`; `smooth: { "up", "down" }` rises and falls at different speeds | `shaping` |
| Layer as texture | `{ "layer": "<id>" }` for a `texture` or `mask` parameter of an effect: the layer is drawn on its own (even with `visible: false`) and fed to the effect; layers may not feed themselves through other layers, masks or groups | `layer_textures` |

Timelines may name `events`, at most 64 `{ "t", "name" }` with `t` within the duration and names
of `[a-z0-9_]{1,32}`; each fires once per pass, also when a frame skips over it (on a `pingpong`
timeline once each way), in the order of their time. A `trigger` or a `play_on` of `event:<name>`
starts on it, as if it had started at the very moment of the event even when the frame came later,
so its own events up to that frame, those at 0 included, fire in the same frame, and scripts receive it (`timeline_events`, which every platform runs). A still scene wakes for the
next event only when a trigger, a sound or a script that the platform runs listens to its name. Clips of meshes and models
name events the same way.

Triggers may also be `click:<layer>`, `enter:<layer>` and `leave:<layer>` for a layer that is
`solid` or has `hit` bones, evaluated without scripts (`layer_events`; `click:` needs the `clicks`
input, the others `pointer`), and `media:play`, `media:pause`, `media:track` (a new track) or
`media:cover` (a new cover) (`media_info`, input `media`).

`media` values follow what the media player of the computer plays, only while the scene is shown,
the package declares `media` and the user allows it: the texts `title`, `artist`, `album`,
`album_artist`, `subtitle`, `genres` and `kind`; the numbers `position`, `duration` (seconds) and
`progress` (0 to 1); `playing` as a flag or number; and the colors `primary`, `secondary` and
`tertiary` of the cover, `text` (readable over the primary color) and `contrast` (black or white).
Texts are empty and colors those of the placeholder while nothing plays.

## Effects

An effect use is `{ "effect", "enabled", "mask", "params", "optional" }`. The built-in effects are
the descriptors in `crates/wprs-scene/effects/`, which list every parameter with its type, range
and default. `texture` parameters take the id of an image asset and `lut` parameters the id of a
`.cube` asset, both as literals. The mask of an effect is a texture; layer and shape masks are
refused there and belong on the layer. At lower quality levels (on battery, with software
rendering or when the frame budget is exceeded) medium and heavy effects run their inner passes at
half or a quarter of their size, use the cheaper settings of their descriptor or are dropped,
`optional` ones first; the pass that writes the result of an effect keeps its full size. An effect
dropped at the current level needs no input, and neither does one whose `enabled` or input
parameter is bound to a property that currently turns it off.

Positions such as `center` and `source` are fractions of the layer (of the canvas on an adjustment
layer, which also places its masks and images there), and a `follow` flag hands them to the pointer
while it is on the output and eases them back within a quarter second after it leaves.
`tilt_shift` blurs outside a band, or by depth when `depth` names an image. `depth_parallax` shifts
each point by `(depth - focus) × strength` pixels towards the pointer (`follow`) or along a slow
orbit (`orbit`, which also takes over while the pointer is away), searching its `depth` image (a
`linear` asset, bright is near) in 8 to 16 `steps`, fewer at lower quality levels, so that near
parts slide over far ones without tearing; `overscan` enlarges the layer to keep its edges out of
sight. Still images with a depth map move with the same search. `glitch` stays still until
`bursts` or `beat` is set (a beat keeps it on for `length` seconds), and `film_grain` only changes
over time with an `fps` above zero.

### Writing an effect

A built-in effect is two files in `crates/wprs-scene/effects/`, found when the crate is built:
`<id>.json`, its descriptor, and `<id>.wgsl`, its fragment entry points. Nothing else changes.

```json
{
  "id": "nudge",
  "name": { "en": "Nudge" },
  "params": [{ "id": "distance", "type": "float", "default": 8, "min": -64, "max": 64 }],
  "mask_mode": "strength",
  "fuse": "uv",
  "reach": "distance"
}
```

```wgsl
fn uv_nudge(uv: vec2<f32>) -> vec2<f32> {
    return uv - px(param(0u).x) * vec2<f32>(1.0, 0.0) * mask_at(uv);
}
```

| Field | Meaning |
|-------|---------|
| `uses` | `time`, `cursor`, `audio`: the scene keeps rendering while time runs, the pointer moves or sound plays; otherwise the effect costs nothing after its first frame |
| `pace` | when a time effect changes: `{ "rate": "<float parameter>" }` (that many times a second), `{ "sweep": ["<speed>", "<gap>", "<phase>"] }` (a band crosses the layer once every `1 + gap` turns and waits off it in between) or `"outside"` (only while the pointer is off the output); the scene then sleeps until the picture changes |
| `mask_mode` | `mix` (default) blends the result with the input by the mask; with `strength` the shader scales itself by `mask_at(uv)`, as distortions should |
| `params` | at most 16 of `float`, `vec2`, `color`, `bool`, `texture` and `lut`, with `id`, `default`, `min`, `max` and `uses` (`time`, `cursor`, `clicks` or `audio`; clicks come with the pointer), an input needed only while the parameter is not zero (a `follow` flag needs the pointer only when set, a `vec2` while either part is not zero); parameter *n* is `param(nu)`, in `.x`, `.xy` or `.rgba` (linear) |
| `passes` | `{ "entry", "scale", "inputs", "output" }`; `scale` is a fraction of the layer size, and lower quality levels shrink every pass but the last; `inputs` (default `["previous"]`) name `previous`, `input`, `source` (the layer before its effects), `mask`, `pass:N`, `state` or `asset:<texture parameter>`, bound to `input_tex` and then `aux0` to `aux3` |
| `cost` | `{ "taps", "passes", "scale" }`; taps × passes × scale² units, light up to 1, medium up to 6, heavy above |
| `fuse` | `uv` or `color` for single-pass effects, see below |
| `reach` | a float parameter, in pixels, by which the effect grows the layer bounds |
| `state` | `{ "scale", "settle" }`, persistent textures for simulations, see below |
| `solve` | `homography`: the first four parameters are `vec2` corners (top left, top right, bottom right, bottom left, as fractions of the layer) and the three slots after the parameters hold the rows of the matrix that takes a layer point back to the unit square; such an effect is not fused |
| `quality` | overrides per level, `{ "q1": { "params": { "taps": 8 }, "scale": 0.5 } }`, applied at that level and below; a number is an upper limit for the value the scene sets, other values replace it, and `scale` shrinks the inner passes and the state |

Every shader starts from one prelude. `frame` holds `time`, `dt`, `resolution`, `cursor` and
`cursor_delta` (fractions of the output), `buttons` (the pressed pointer buttons as a bit mask),
`inside` (1 while the pointer is on the output), `presence` (`inside` eased over a quarter second),
`bass`, `mid`, `treble`, `level`, `beat`,
`since_beat` (seconds since the last beat), `seed`,
`quality` and 64 audio bands (`band(i)`). The samplers are `samp` (clamped) and `wrap` (repeating);
`lut_tex` is the 3D texture of the first `lut` parameter, read with `lut(rgb)`. Helpers: `tap(uv)`,
`sample(t, uv)`, `px(pixels)`, `mask_at(uv)`, `layer_uv(uv)` and its inverse `from_layer(uv)`,
`asset_at(t, uv)` (an asset placed on the layer, such as a depth map), `cursor_uv()`,
`screen_uv(uv)`, `to_linear`, `from_linear`, `straight`, `oklab`, `from_oklab`, `gauss`, `hash`,
`hash2`, `noise1`, `value_noise`, `gradient_noise`, `fbm(p, octaves)`, `bayer(pixel)` and
`relief(depth, sampler, uv, shift, focus, steps)`, the depth search of `depth_parallax`.
`fx.texel` is the size of the first input, `fx.info.z` the pass index and `fx.area` the effect
rectangle in output pixels. Effects may not declare bindings of their own.

An effect with `fuse` defines `fn uv_<id>(uv) -> vec2<f32>`, where its output reads its input, or
`fn color_<id>(color, uv) -> vec4<f32>` on premultiplied colors, and gets the single pass
`fs_<id>`. Consecutive light fused effects of a layer without masks and with a fixed `enabled` run
as one generated pass, compiled when the scene loads, so their helper functions start with the
effect id.

`"state": { "scale": 0.25, "settle": 3 }` gives the effect two textures (16-bit float where the GPU
allows) at that fraction of the layer size, kept between frames. A pass with `"output": "state"`
writes the next one once per 1/60 s of scene time (at most four times a frame, so a simulation
keeps its speed at any frame rate; `fx.info.y` counts these steps) and `state` reads the latest.
They start transparent, restart when the layer size changes and are dropped when the scene leaves,
stops or pauses. `settle` keeps rendering that many
seconds after the pointer moves, a button is pressed or the sound stops, so ripples fade before the
scene goes still.

`cargo test -p wprs-scene` checks every descriptor and compiles its shader. A GPU test for an
effect goes in `crates/wprs-render/tests/effects/<id>.rs`, also found automatically:

```rust
use super::*;

#[test]
fn nudge_moves_the_image() {
    let Some(out) = Fx::new("nudge", json!({ "distance": 8 })).input(halves(RED, BLUE)).shot() else { return };
    out.check(&[((36, 16), RED), ((44, 16), BLUE)]);
}
```

`Fx::chain(effects)`, `.adjust()` (effects on an adjustment layer), `.image(id, image, linear)`,
`.lut(id, text)` and `.quality(level)` set up the scene. `.run()` returns a `Run` whose `runtime`
(time, audio) and `pointer([x, y])` change between `shot()`s, and whose `stats()` count passes and
new textures. A `Shot` has `at`, `check`, `mean` and `diff`.

### Package effects

A package may bring at most 32 effects of its own (`custom_effects`): `effects` in `scene.json`
maps an id (`[a-z0-9_]{1,32}`, not the id of a built-in effect) to `{ "descriptor": "<path>.json",
"wgsl": "<path>.wgsl" }`, two files of the package of at most 64 KiB each (256 KiB of shaders for
all of them), and layers use the id
like any effect. Readers parse the descriptor with the fields above, require its `cost`, compile
the shader with the contract below and refuse it with `/effects/<id>: <reason>` when it does not
compile, lacks an entry point or an `override` its options name, or declares bindings of its own;
then they check every use of it like the uses of built-in effects. The inputs an effect needs are
found in the compiled shader (reading `frame.time`, the pointer or the audio, also through helper
functions) and added to its `uses`, so that the scene asks for them without the author listing
them. Package descriptors may also use `stage`: `material` for the shader of a model material, whose
WGSL defines `fn material(s: Surface) -> Surface`, or `particle` for a particle renderer
(`render.shader`), whose WGSL defines `fn particle(p: Sprite) -> Sprite` for each corner of each
particle; such descriptors have no passes, and the scene may only use them where their stage fits.
`Surface` has `position`, `normal`, `uv`, `albedo`, `emissive`, `roughness`, `metallic` and
`occlusion`; `Sprite` has `position`, `velocity`, `corner`, `uv`, `color`, `size`, `rotation`, `age`
and `life`. Package descriptors may also use:

| Field | Meaning |
|-------|---------|
| `params` | at most 24; besides the kinds above, `int` (whole numbers), `enum` (with `values`: 1 to 64 `{ "value", "label" }`), `blend` (one of the 32 blend modes, given to the shader as its number in the order of the list), `position` (a point of the layer the editor lets the author place) and `mask` (a picture painted per channel, with `channels` 1, 3 or 4 and `paint`, the color or number unpainted parts have); every parameter may have a `label` and a `help` per language, of at most 256 characters |
| `targets` | at most 8 named textures `{ "scale" (0 to 2 of the layer size), "format" }` with `rgba8` (default) or `rgba16f`, kept between passes of one frame |
| `options` | at most 8 variants `{ "id", "values", "default", "when_bound" }`, each a WGSL `override` of the shader with 1 to 16 whole numbers or flags; a use sets them through `params`, as literals or bound to a property, and every combination it may need (at most 8 per use) is built when the scene loads, never during a frame; `when_bound` turns an option on while the named texture parameter has a picture |
| pass `target` | writes the pass into that target instead of the next texture |
| pass `copy` | a pass without a shader that copies one input (`"copy": "target:history"`) |
| pass `blend` | `replace`, `normal`, `add` or `multiply` onto its output, to accumulate without a copy |
| pass `vertex`, `grid` | a vertex entry point and a grid of 1 to 64 cells a side over the layer, to bend it (`vertex_effects`) |
| pass `inputs` | at most 9, also `backdrop` (what is already drawn under the layer, which then draws on its own), `target:<name>` and `asset:<mask parameter>` |

The contract is the prelude above plus `aux4` to `aux7` (eight auxiliary textures in all), `PI`,
`tex_size(i)` (size in pixels of `aux<i>`), `daytime()` (0 at midnight to 1), `parallax()` (the
current parallax shift), `layer_color()` and `layer_alpha()` (the color and opacity of the layer),
`band_left(i)` and `band_right(i)` (64 bands per channel), `band_of(i, n)` (band `i` of 16, 32 or
64), `rotate2(v, degrees)`, `greyscale(rgb)`, `hsv(rgb)`, `from_hsv(hsv)` and `blend_mode(mode, b,
s, amount)`, which blends straight colors with any of the blend modes. `CONTRACT` in
`crates/wprs-scene` holds it, and package shaders are checked against it whole. Package effects run
the same WGSL on every platform.

### Effects not drawn yet

The format also describes the effects and parameters below, so that a scene can be written with
them today. Each is validated like a built-in one (types, ranges, assets, `{ "layer" }` textures
and the inputs it needs) and names a feature: `effect.<id>` for an effect, `effect.<id>.<parameter>`
for a newer parameter of an existing effect. Engines refuse a package that uses one they do not
draw yet with the newer-version message, on every platform, and run it once the effect gains its
shader; the names stay the same. Package effects may not take their ids. Positions are fractions
of the layer, `blend` parameters take any of the 32 blend modes, `phase` parameters are `linear`
pictures whose brightness shifts the time of each point so that neighbouring parts do not move
together, and `gradient` parameters are pictures of one row read from left to right.

| Effect | What it does | Parameters |
|--------|--------------|------------|
| `blend_gradient` | mixes in another picture through a gradient map | `texture`, `map` (textures), `progress`, `softness` (0 to 1) |
| `blend_texture` | blends another picture, a layer or a video over the layer; a layer may chain up to six | `texture`, `blend`, `amount` (0 to 1), `offset` (−2 to 2), `scale` (0.01 to 16) |
| `caustics` | moving light patterns of water | `color`, `scale` (0.1 to 16), `speed` (0 to 8, needs time), `strength` (0 to 4), `perspective` (0 to 1) |
| `displace` | bends the layer by a normal or offset map, or by built-in noise without one | `map`, `mode` (0 normals, 1 offsets), `scale` (0.1 to 16), `strength` (0 to 256 px), `scroll` (−4 to 4, needs time while not zero), `ratio` (0.1 to 10) |
| `electric` | thin crackling lines from two drifting noises | `color_a`, `color_b`, `bounds` (0 to 1), `scale` (0.1 to 16), `smoothness` (0 to 1), `speed` (0 to 8, needs time), `multiply` (0 to 8), `blend` |
| `fire` | moves and colors flames already in the picture | `speed` (0 to 8, needs time), `strength` (0 to 64 px), `scale` (0.1 to 16), `gradient` |
| `fluid` | smoke, fire or liquid simulated over the layer and pushed by the pointer | `render` (0 gradient, 1 emitter color, 2 background color, 3 distortion), `gradient`, `color`, `velocity_decay`, `dye_decay` (0 to 1), `push` (0 to 8, needs the pointer while not zero), `source`, `source_angle` (−360 to 360), `source_strength` (0 to 64), `line_from`, `line_to`, `line_strength` (0 to 64), `blend`, `perspective` (0 to 1), `opaque` |
| `glitter` | short sparkles in a grid of cells | `density` (0 to 1), `size` (0.5 to 16 px), `speed` (0 to 8, needs time), `color` |
| `lens_flare` | ghosts and a halo along the line from a light to the center | `source`, `follow` (needs the pointer when set), `intensity` (0 to 4), `ghosts` (0 to 8), `halo` (0 to 1), `color` |
| `light_shafts` | animated beams of light of their own shape and color | `direction` (0 linear, 1 radial, 2 corner), `angle` (−360 to 360), `center`, `color`, `gradient`, `exponent` (0.1 to 8), `intensity` (0 to 4), `feather`, `smoothness` (0 to 1), `speed` (−8 to 8, needs time while not zero) |
| `local_contrast` | sharpens local contrast | `radius` (1 to 64 px), `amount` (0 to 4), `luma_only` |
| `pulse` | makes brightness, color or opacity beat in time or with a band of sound | `what` (0 brightness, 1 color, 2 opacity), `speed` (0 to 16, needs time), `bounds` (0 to 4), `tint_low`, `tint_high`, `noise`, `noise_speed` (0 to 16), `phase`, `audio_band` (0 off, or a band 1 to 64 that needs sound) |
| `reveal` | shows another picture through a shape that may follow the pointer | `texture`, `shape` (0 circle, 1 ellipse), `radius`, `softness` (0 to 1), `follow` (needs the pointer when set), `invert`, `center` |
| `spin` | turns the part of the layer around a center | `center`, `angle` (−360 to 360), `speed` (−720 to 720 °/s, needs time while not zero), `radius` (0 to 2) |
| `tint` | tints or colorizes the layer with a blend mode | `color`, `blend`, `amount` (0 to 1), `mode` (0 tint, 1 colorize, which keeps the brightness) |
| `trail` | keeps fading copies of earlier frames, independent of the frame rate | `persistence` (0 to 0.99) |
| `transform` | moves, scales, turns or skews the layer after the effects before it | `offset` (−2 to 2), `scale` (0.01 to 16), `angle` (−360 to 360), `skew` (−89 to 89), `pivot` |
| `vhs` | a tracking line, noise and color fringes of a worn tape | `strength` (0 to 1), `speed` (0 to 8, needs time), `noise` (0 to 1), `aberration` (0 to 16 px) |

| Effect | Newer parameters |
|--------|------------------|
| `blur` | `blend`, `amount` (0 to 1) |
| `chromatic_aberration` | `mode` (0 expansion, 1 directional, 2 radial, 3 barrel), `angle` (−360 to 360) |
| `film_grain` | `mono`, `bias` (−1 to 1, towards dark or bright parts), `blend` |
| `flow` | `noise` (a built-in flow field when no map is given), `phase` |
| `god_rays` | `mode` (0 radial, 1 directional, 2 star), `angle` (−360 to 360), `noise` (0 to 1), `edges` (2 to 16 rays of a star) |
| `iris`, `lens_distort` | `background` (shows what is behind where the layer moved away) |
| `reflection` | `angle` (−360 to 360) of the mirror line |
| `scroll` | `repeat_x`, `repeat_y` |
| `shake` | `noise`, `friction` (0 to 1), `bounds` (0 to 1), `phase` |
| `shine` | `blend`, `gradient` |
| `sway` | `double`, `phase` |
| `twirl` | `aspect` (0.1 to 10), `inner`, `noise` (0 to 1), `repeat` |
| `water_ripple` | `perspective` (0 to 1), `reflection`, `tint`, `walls` (a picture where the water stays still), `speed` (0 to 4) |
| `waves` | `count` (1 or 2), `angle2` (−360 to 360), `perspective` (0 to 1), `phase` |

## Sound

```json
{ "id": "gusts", "type": "sound", "sound": "wind", "loop": true, "fade_in": 3, "fade_out": 3,
  "play": { "prop": "wind", "map": { "calm": false, "breeze": true } },
  "volume": { "timeline": "storm", "keys": [[0, 0.4], [20, 1, "in_out_sine"], [40, 0.4]] }, "pan": -0.3 }
```

A `sound` layer plays a sound asset: `.ogg` (Vorbis or Opus), `.opus`, `.flac` or `.wav`, mono or
stereo, sampled at 8 to 192 kHz, at most 8 MiB and 60 s long, and at most 180 s for all the sounds
of a scene together; the package reader checks the headers and the daemon stops decoding a sound
that goes on longer than its header says (an MP3 without a length in its header may last up to the
60 s). A scene has at most 8 sound layers, which mix together.

Sound follows the time of the scene. Each layer knows how long ago the scene started it: since the
scene started for a `play` that is always on, since the key that turned it on for `play` keyframes
(on a looping timeline, since the start of the pass), since the trigger for `play_on`, and since it
turned on for the rest. The sound plays at that point of its clip or of its list, so a loop that
drifts more than 60 ms from the scene jumps back in step with a 5 ms fade, a scene that resumes
plays from where its timelines are, and a sound started again starts over. About 50 ms pass between
the scene and the speaker (the soundtrack asks PipeWire for 20 ms periods and keeps about 20 ms
queued).

| Field | Meaning |
|-------|---------|
| `loop` | `true` (default) repeats the sound without a gap; `false` plays it once each time `play` turns on |
| `play` | starts the sound when it turns on and stops it when it turns off (default `true`); the sound plays from the point the scene says, see above |
| `volume` | `0` to `1` (default `1`); changes glide over 20 ms, so keyframed curves sound smooth |
| `pan` | `-1` (left) to `1` (right), default `0`, gliding like `volume` |
| `fade_in`, `fade_out` | seconds (`0` to `60`) the sound takes to rise when it starts and to fall when it stops |

Newer sound layers may also use (`sound2`):

| Field | Meaning |
|-------|---------|
| `sound` as MP3 | `.mp3` files (MPEG Layer III) whose first frame follows the ID3 tag, checked like the others; only a file named `.mp3` is read as MP3 |
| `sounds` | instead of `sound`, a list of 1 to 16 sound assets |
| `mode` | with `sounds`: `sequence` (default, in order) or `random` (picked with the scene seed) |
| `pause` | `[min, max]` seconds (0 to 600) of silence between clips, picked with the scene seed |
| `stream` | `true` plays the files as they are read instead of from memory, for long sounds: at most 32 MiB and 10 minutes each and 64 MiB for all the streamed sounds of a scene, outside the 180 s of the others |
| `play_on` | a trigger (`start`, `beat`, `prop:<id>` or `event:<name>`) that starts the sound, always under the user's volume and mute |

A layer has either `sound` or `sounds`. With `loop: false`, `sequence` plays the list once and
`random` one clip each time it starts. `random` never plays the same clip twice in a row when the
list has more than one, and the clips and pauses it picks follow the scene seed, so the same scene
with a fixed `clock.seed` plays the same order. A streamed sound is decoded a little ahead of where
it plays, and the next streamed clip of a list starts decoding 1.5 s before its turn, so lists and
loops of long sounds play without gaps; nothing of a stream is kept once it has played. `play_on`
starts the sound again each time its trigger fires; `play` still mutes it while it is off. The
desktop plays these sounds; Android plays scenes without sound (`sound` and `sound2` are left out
there).

`play`, `volume` and `pan` may be literals or follow a user property or keyframes on the scene
clock, which a timeline triggered by a property or a beat can drive; they never follow the audio
analysis, the pointer, a clock text or a script, so a scene cannot hear its own sound. Values that
change over time are read at the frame rate of the output without rendering a frame. A sound layer
draws nothing, takes none of the visual fields (`transform`, `opacity`, `effects` and so on) and
cannot be a mask.

Every scene with sound has a `volume` user property, a slider from 0 to 100 % (100 by default)
added automatically; a manifest may declare it to change its label, default or order, as long as
it stays a slider within 0 to 100. Scenes start muted like videos and follow `playback.muted` and
`playback.volume`; only one output plays the sound of a scene shown on several outputs, and nothing
plays on the lock screen (see "Scene sound" in `dbus.md`).

## Particles

`particles` maps at most 8 ids to systems, which hold at most 200 000 particles together. A
`particles` layer draws one system; several layers may draw the same system, which then moves only
once per frame and shares its budget between them. Positions are canvas pixels, and the transform of the layer moves, turns and
scales the whole system around the canvas center.

```json
"particles": {
  "sparks": {
    "capacity": 4000, "seed": 7, "prewarm": 2.0,
    "emitters": [{ "shape": { "box": [[0, 1080], [1920, 1100]] }, "rate": { "prop": "density" },
                   "burst": { "count": 40, "on": "beat" } }],
    "init": { "life": [2, 5], "speed": [40, 120], "angle": [250, 290], "size": [4, 12], "color": ["#ffd27a", "#ff7a3d"] },
    "forces": [{ "gravity": [0, -20] }, { "turbulence": { "strength": 30, "scale": 0.004 } },
               { "attractor": { "target": "cursor", "strength": 5000 } }, { "drag": 0.4 }],
    "collide": [{ "bounds": "canvas", "bounce": 0.3 }],
    "render": { "texture": "spark", "frames": "life", "size_over_life": [[0, 0], [0.1, 1], [1, 0]],
                "trail": { "length": 8, "width": 0.5 } }
  }
}
```

| Field | Meaning |
|-------|---------|
| `capacity`, `seed`, `prewarm` | 1 to 200 000 particles; the seed of the system, mixed with `clock.seed`; seconds simulated when the system first shows (at most 10) |
| `emitters` | at most 4 of `{ "shape", "rate", "burst", "follow", "inherit" }`. Shapes are `{ "point": [x, y] }`, `{ "box": [[x0, y0], [x1, y1]] }`, `{ "circle": { "center", "radius", "inner" } }` (a ring when `inner` is above 0), `{ "line": [[x0, y0], [x1, y1]] }`, `"edge"` (the border of the canvas) and `{ "mask": { "texture", "threshold" } }` (where the alpha of an image stretched over the canvas is above `threshold`, 0.5 by default). `rate` is particles per second and may be bound like any value. `burst` emits `count` particles every `every` seconds, `on` each `beat` or `click`, or once when the system starts. `follow: "cursor"` moves the shape with the pointer and `inherit` adds that fraction of the pointer velocity |
| `init` | `[min, max]` ranges picked for each particle: `life` (seconds), `speed` (pixels per second), `angle` (clockwise degrees, 0 to the right), `size` (pixels), `rotation`, `spin` (degrees per second), `frame` (first atlas frame) and `color` (two colors mixed at random) |
| `forces` | at most 8 of `{ "gravity": [x, y] }` (also `wind`), `{ "drag": k }`, `{ "turbulence": { "strength", "scale" } }` (curl noise), `{ "attractor": { "target", "strength" } }` (negative strengths repel), `{ "vortex": { "center", "strength" } }`, `{ "orbit": { "center", "strength" } }` and `{ "noise": strength }`. Gravity and strengths may be bound, as in `{ "noise": { "audio": "bass", "scale": 300 } }`. Targets and centers are a point or `"cursor"`, which pulls nothing while the pointer is away |
| `collide` | at most 4 of `{ "bounds", "bounce", "friction", "kill" }` where `bounds` is `"canvas"`, `{ "plane": { "point", "normal" } }` (particles stay on the side the normal points to) or `{ "circle": { "center", "radius" } }` (particles stay outside). `bounce` and `friction` go from 0 to 1 and `kill` removes a particle when it touches |
| `render` | `texture` (an image asset, split into frames by its `atlas: [rows, columns]`), `frames` (`time` at `fps`, `life` or `random`), `mix` (blends one frame into the next), `size_over_life` and `color_over_life` (`[t, value]` keys from 0 to 1, at most 64, that multiply size and color), `stretch` (seconds of motion a particle is stretched along) and `trail` (`length` from 1 to 16 steps, `width` from 0 to 1). Without a texture particles are soft discs. The layer's `blend` (`add` suits glowing particles), `opacity`, `mask` and effects apply |

The simulation advances in fixed steps of 1/60 s, at most 15 per frame; a longer frame takes 15
equal steps up to a second, so particles keep real time down to one frame per second and never
catch up more than a second, and the same seed and the same frames give the same particles. While
no particle lives and none is emitted, the system skips ahead to its next timed burst. A paused
scene frees its particles and starts them again, prewarmed, when it resumes. It runs in compute shaders
on the GPU, and on the CPU with software rendering or where compute shaders are missing. Particles
only move while their layer is shown and the scene runs, and a scene keeps rendering only while
particles live, are emitted or wait for their next timed burst. A scene holds at most 2 000
particles with software rendering, 5 000 on other devices without compute shaders and 10 000,
50 000 or 200 000 on the GPU depending on the device; the budget is shared by capacity, particles
are drawn smaller where they would cover the screen more than 1, 1.5, 3 or 6 times over, and trails
are limited to 0, 4, 8 or 16 steps. Lower quality levels let only 75 %, 50 % or 25 % of the
particles be born and halve or drop trails. `wprs-ctl scene bench` reports where particles run,
their slots, how many are alive at the end, the steps per frame and the simulation time after the
warm-up frames.

### Newer particle fields

Systems may also use these fields. Most name `particles2`, which also covers scenes beyond the
older limits: up to 32 systems, 8 emitters, 16 forces and 8 colliders per system and 30 s of
`prewarm`. Control points are named `p0` to `p7`: `p0` is the origin of the system unless `points`
sets it, and every other name must be one of `points`. Targets and centers of forces, `around`,
`between` and `init.inherit` take a point, `"cursor"` or a control point.

| Field | Meaning | Feature |
|-------|---------|---------|
| `space` | `local` (default) or `world`: in `world` the transform of the layer and of control points only places particles when they are born | `particles2` |
| `points` | at most 8 control points `{ "at" (may be bound), "follow": "cursor", "space", "angle" }`, `at` or `follow` | `particles2` |
| `lock` | settings layers may not change through `instance` (`alpha`, `size`, `count`, `speed`, `life`, `time_scale`, `brightness`, `color`, `points`) | `particles2` |
| emitter `duration`, `delay` | seconds after which the emitter stops (0 never) and before it starts (0 to 86400) | `particles2` |
| emitter `periodic` | `{ "on": [min, max], "off": [min, max] }` seconds of emission and pause, picked from the seed | `particles2` |
| emitter `max_per_step`, `audio_gate` | at most that many births per 1/60 s step (1 to 1000), and no births while the sound is silent | `particles2` |
| shape `box` | also `{ "from", "to", "inner": [w, h], "at" }`, a hollow box around a control point | `particles2` |
| shape `circle` | also `scale: [x, y]` (0.01 to 100), `sign: [x, y]` (−1, 0 or 1: half or quarter circles) and `at` | `particles2` |
| shape `layer` | `{ "id", "threshold", "color", "inherit", "jitter", "refresh": "static" \| "on_change" }` births where a layer is opaque, taking its color and motion | `particle_layers` |
| shape `around` | `{ "at", "count" (1 to 4096), "arc", "radius", "speed", "mode": "repeat" \| "mirror", "restart" }` births in turn around a point | `particle_fields` |
| shape `between` | `{ "from", "to", "count", "span", "mode", "arc": { "amount", "side" }, "taper": { "position", "speed", "size" } }` births along a line or arc between two points | `particle_fields` |
| `init` ranges | each range may add an exponent `[min, max, exponent]` (0.1 to 10) that leans the pick to one end; `color` too | `particles2` |
| `init.alpha`, `hsv`, `palette` | an opacity range of its own; `{ "hue": [0, 360], "steps", "sat", "val" }`; or `{ "colors" (1 to 10), "noise": [h, s, v] }` (not both), which replace `color` | `particles2` |
| `init.velocity`, `radial`, `inherit` | `[[vx0, vy0], [vx1, vy1], exponent?]` replacing `speed` and `angle`; a speed away from the center of the shape; `{ "from" (cursor or control point), "speed": [min, max] }` | `particles2` |
| `init.turbulent`, `offset_noise`, `remap` | turbulent starting velocity `{ "forward", "scale", "noise_speed", "offset", "speed", "phase", "audio" }`; a fractal offset of the birth `{ "distance", "scale", "sign", "octaves" (1 to 6), "noise_scale", "noise_speed" }`; at most 8 `{ "input", "in", "clamp_in", "output", "out", "clamp_out", "op", "wave" }` that map `daytime`, `runtime`, `life`, `age`, `speed`, `random`, `distance:pN`, `audio:<band>` or `cursor:x\|y` onto `size`, `alpha`, `color`, `speed`, `life`, `rotation`, `spin` or `frame` with `set`, `mul`, `add` or `sub` and an optional `sine` or `noise` wave | `particle_fields` |
| `perspective`, `init.depth`, `init.depth_speed` | `{ "focal" (1 to 100000 px) }` and depth ranges in pixels, which need `perspective` | `particle_depth` |
| force `window` | `[in_start, in_end, out_start, out_end]` fractions of the life, in order, in which any force fades in and out | `particles2` |
| forces | `{ "oscillate": { "what": "position" \| "alpha" \| "size", "freq", "scale", "phase", "mask" } }`, `{ "cap": speed }`, `{ "torque": k }`, `{ "spin_drag": k }`, `{ "slow_near": { "target", "inner", "outer", "factor" } }`; `attractor`, `vortex` and `orbit` take `radius` and `falloff` (`constant`, `linear`, `inverse_square`), `vortex` also `inner`, `outer` and `ring: { "radius", "width", "pull" }`, `turbulence` also `speed`, `time_scale` and `mask` | `particles2` |
| forces | `{ "keep_distance": { "target", "distance", "strength" } }`, `{ "keep_between": { "from", "to", "strength" } }`, `{ "remap": { … } }` applied every step (input `age` too) | `particle_fields` |
| force `boids` | `{ "radius", "separation", "alignment", "cohesion", "max_speed" }` flocking, for systems of at most 8192 particles | `boids` |
| colliders | `{ "box": { "from", "to", "inside" } }`, `{ "segment": { "from", "to" } }`, `circle` with `inside`, and `stick: true` | `particle_collisions` |
| collider `layer` | `{ "layer": { "id" } }`, the opaque shape of a layer | `particle_layers` |
| `children` | at most 4 `{ "system", "on": "static" \| "spawn" \| "death" \| "follow", "probability", "max", "offset", "angle", "scale" (0 to 100), "points_from", "inherit", "follow_inherit" }`: other systems born with or from each particle, at most two levels deep and never back to themselves; `inherit` copies `color`, `size`, `alpha` or `velocity`, every step with `follow_inherit` on `follow` children; a collider that `kill`s counts as a death | `particle_children` |
| render `brightness`, `cycles`, `stretch` | 0 to 16, may be bound; repeats the frames that many times per life (1 to 64, with `frames: life`); `stretch` may be `{ "time", "min", "max" }` in pixels, and `min` equal to `max` only turns a sprite along its motion | `particles2` |
| render `rope`, `trail` | `{ "subdivide" (0 to 7), "width", "uv_scale", "uv_scroll", "smooth" }` joins particles in birth order; trails of up to 64 steps with `segments`, `subdivide`, `uv_scale`, `uv_scroll` and `textured` | `particle_ropes` |
| render `cutout`, `orient`, `angle` | `[start, end, opacity]` remaps the alpha of the texture; `screen`, `velocity` or `fixed` with `angle` | `particle_looks` |
| render `lit`, `normal`, `refract` | lit by the scene lights with a normal map; `{ "normal", "amount" }` bends what is behind | `particle_lighting` |
| render `shader` | a package effect with `stage: "particle"` | `particle_shaders` |

A `particles` layer may also set `instance` (`alpha`, `size`, `count`, `speed`, `life`, `time_scale`,
`brightness`, `color` and `points`, which may be bound and multiply or move what the system does for
that layer only, unless the system locks them) and a `render` of its own that replaces the one of
the system (`particles2`). Each end of the `init` ranges of numbers (`life`, `speed`, `angle`,
`size`, `rotation`, `spin`, `frame`, `alpha`, `radial`, `depth`, `depth_speed`) and the numbers of
emitter shapes (`radius` and `inner` of `circle`, `threshold` of `mask` and `layer`, `arc`, `radius`
and `speed` of `around`, `span` of `between`) may be bound like any value, for instance
`"size": [2, { "audio": "bass", "scale": 8 }]` (`particle_values`); colors and velocities stay
literal. Births may also follow the sound or the pointer through the `rate` of the emitter, the
`instance` multipliers of the layer or control points.
## User properties and presets

User properties (settings) are declared in the manifest, under `properties` (id → definition), and
scenes refer to them by id. Each has a `type` and may set `label`, `help`, `group`, `visible_if`,
`enabled_if`, `order`, `icon` and, for sliders, combos and flags, `quality`:

| `type` | Fields | Value |
|--------|--------|-------|
| `color` | `default`, `alpha` | `#rrggbb` or `#rrggbbaa` |
| `slider` | `min`, `max`, `default`, `step`, `decimals` (0 to 6), `integer`, `unit`, `editable`, `log` | number, clamped to the range, aligned to the step and rounded to the decimals |
| `bool` | `default` | `true` or `false` |
| `combo` | `options` (values or `{ "value", "label", "icon" }`, at most 64), `default` | one of the option values: texts, numbers or `true` and `false`, all of one kind |
| `text` | `default`, `max_len` (≤ 1024), `multiline`, `secret` | text |
| `file` | `accept` (`image`, `video`), `default_asset` | absolute path of a file chosen by the user, or `null` |
| `folder` | `accept`, `pick` (`random`, `sequence`), `every` (10 to 86400 s), `mode` (`one`, `all`), `max_files` (1 to 1000, default 1000), `recursive` (0 to 2 levels of subfolders, default 0) | absolute path of a folder chosen by the user, or `null` |
| `shortcut` | | what the user chose to open: `{"uri": "https://…"}`, `{"file": "/…"}`, `{"folder": "/…"}` or `{"app": "<desktop id>"}`, or `null` |
| `button` | | none; pressing it is an event |
| `group` | `collapsed` | none; the properties that name it are listed under it |
| `label`, `divider` | | none; a line of text, or a line across the form with an optional label (a divider without one shows no text) |

A slider without `step` moves in steps of its decimals, which come from `decimals`, else from
`step`, else two (none for `integer`); its value is always rounded to them. `editable` adds a box to
type the number and `log` spreads the range logarithmically (it needs `min > 0`). `order` is any
number: properties sort by it and then by id, and the members of a group follow it in their own
order; a group cannot be in another group. An option of a combo with an empty label shows its value.
`icon` is one of `appointment-soon`, `audio-volume-high`, `audio-volume-muted`, `audio-x-generic`,
`camera-photo`, `dialog-information`, `document-open`, `edit-find`, `emblem-favorite`, `folder`,
`go-home`, `image-x-generic`, `input-keyboard`, `input-mouse`, `media-playback-start`,
`media-playlist-repeat`, `media-playlist-shuffle`, `preferences-desktop-theme`,
`preferences-system`, `text-x-generic`, `video-display`, `video-x-generic`, `view-refresh`,
`weather-clear`, `weather-clear-night`, `weather-few-clouds`, `weather-overcast`, `weather-showers`,
`weather-snow`, `weather-storm`, `x-office-calendar` or `zoom-in`, symbolic icons of the desktop
theme; other names of `[a-z0-9-]` are accepted and not shown, so that newer versions can add icons.
Packages never bring their own icons.

`visible_if` hides a property and `enabled_if` greys it out while a condition does not hold, and a
group's conditions apply to its members. A condition is a map `{ "<other id>": value }` (all must
match) or an expression of a small language, never code: property ids, numbers, texts in `'…'` or
`"…"`, `true`, `false`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!` and parentheses, such as
`"mood == 2 || speed > 1.5"`. An expression has at most 256 characters and 8 levels of nesting;
it may compare a property only with a value of the same kind (`<` and the like only numbers), and
the validator refuses unknown ids, properties without a value, conditions that are not true or
false and conditions that depend on themselves through other properties.

A `button` holds no value: pressing it (`PressSceneButton`, `wprs-ctl prop press`) starts the
timelines whose `trigger` is `prop:<id>` and shows the press to the scripts for one frame, and it
cannot be pressed while its `enabled_if` or that of its group does not hold. Only a property that
holds a value may be followed with `{ "prop" }`, and a `trigger` `prop:<id>` names a button or such
a property. A `secret` text has no default and is kept in the Secret Service on the desktop and
sealed by the Android keystore on the phone, never in `config.toml`, logs, presets, exported
settings or sync; a scene reads it as an empty text, since only content allowed to use the network
may receive it.

A `file` chosen by the user replaces the asset named by `default_asset` wherever the scene uses it
(image and video layers, masks, effect textures and particle textures), and also shows wherever
`{ "prop": "<id>" }` names the property, with or without `default_asset`. Images are decoded like
the scene's own; with `accept: ["video"]` a video plays in image and video layers without sound,
in a loop and paused with the scene, while masks, effect textures and particles keep the package's
own asset. Image and video layers, the `texture` parameters of effects, the `texture` of masks and
the `render.texture` of particles take `{ "prop", "map" }`, so that a combo or a flag switches
between assets of the package (variants), and `{ "prop": "<id>" }` of a `file`; `wprs-ctl pack
check` warns when the images of the variants of one place do not share one proportion. A new
choice replaces only the textures it touches: the clock, the timelines, the particles (unless
their own texture changed) and the scripts of the scene go on. A `folder` shows one of its
images (or videos, with `accept`) in an image layer bound to it with `{ "prop": "<id>" }`, picked
at random or in order and changed every `every` seconds while the scene is shown; only the folder
itself is read (and `recursive` levels of its subfolders), without following links out of it or
reading hidden files, at most `max_files` of them in name order, and nothing is read while the scene is not shown, paused, covered or behind the lock screen: a
turn missed then happens once when the scene shows again. Files and folders are chosen by the user
through the file chooser, are never set by presets and never travel with exported settings or sync.
`mode: all` lets scripts and web pages list every file of the folder (`files`, `onFiles`), where
`one` only gives them the picked one.

A `shortcut` holds what the user chose to open from the wallpaper: an `https://` address (opened
through the OpenURI portal, which asks), a file or a folder (chosen through the file chooser) or an
application by its desktop id, never a command. Only the user sets it, never the author, a preset
or sync, and the content never sees the target: a script or a page asks to open it, only inside a
real click and at most once a second. `quality` maps quality levels (`q0` to `q3`, `q3` the
highest) to a value of the property: on that level and the lower ones the value is at most that
one for a slider and exactly that one for a combo or a flag, so a power-saving mode turns itself on
with the battery.

`presets` is a list of `{ "id", "label", "values" }`; a preset cannot set files, folders, secrets
or buttons. Values are changed through `GetSceneProperties`, `SetSceneProperty`,
`ApplyScenePreset` and `ResetSceneProperties` (`dbus.md`) or `wprs-ctl prop`, and stored in
`config.toml`, once for every screen or per screen: a screen without values of its own follows
and changes the shared ones until it gets its own copy (`ShareSceneValues`, `wprs-ctl prop share`,
*Same settings on every screen* in the app), which it keeps even when emptied by a reset. The
user can also save the current values as a preset of their own, rename, delete, export it to a
small file and import it back (`{"format": "wpak-values/1", "package", "version", "values"}`); saved
presets belong to the package id, survive updates and travel with sync. A stored value whose
property no longer exists, changed its type or falls outside its range after an update of the
package is ignored for the default, with a warning in the log, in `GetSceneProperties`
(`warnings`) and in `wprs-ctl prop ls`. Labels, help, option labels and preset labels are text or
`{ "<language>": text }`, and in `wallpaper-rs.wpak/2` also `"@key"` of the manifest's `strings`;
ids and limits depend on the manifest version (`package.md`), and the newer kinds and fields above
need the second version and its feature `properties2` (`folder` and `user_videos` for folders and
videos chosen by the user, `folder2` for `max_files` and `recursive`, `folder_lists` for
`mode: all`, `shortcut` and `quality_settings`; `package.md` says which platforms run each). The app shows these forms under *Scene settings* for each screen on its
Settings page, and the Android app on the page of the scene. The scenes in the `scenes/` folder of
the repository are examples of this format, dedicated to the public domain.

## Inputs

Audio analysis, the pointer and script outputs reach the renderer as plain structures of
`wprs-scene`: `AudioFrame` (64 bands per channel and their average, `bass`, `mid`, `treble`,
`level`, `beat`, `silent`; all zero while audio is off), `PointerState` (position, smoothed
position, velocity, buttons, `inside`, in fractions of the screen) and `ScriptOutputs` (values of
script sources, layer overrides and timeline commands). A scene only renders again for an input it
uses. The pointer follows the reported position with a critically damped spring that settles in
about 0.15 s and returns to the center when the pointer leaves the screen. Audio and the pointer
need the user's grant for the scene (`dbus.md`, `Inputs1`), which the app and the tray ask for once
when such a scene is shown; a `wallpaper-rs.wpak/2` manifest declares them in `inputs`
(`package.md`), and a scene of such a package receives only the inputs its manifest declares. What
the media player plays (`media` values, cover assets, `media:` triggers and scripts that declare
it) is the `media` input, and clicks on `solid` layers the `clicks` input.

## Scripts

`scripts` lists at most 16 JavaScript modules of the package (`{ "path": "scripts/spin.js" }`). They
run in a separate process without files, network or other processes, with 32 MiB of memory, a
256 KiB stack, 100 ms to load and 4 ms of CPU per frame for all of them. A module may export
`update(frame)`, called in order for every frame while the output is active; what it returns is the
value of `{ "script": "<its path>" }` sources. Results reach the next frame, so a slow script never
holds up rendering. Once the results stay the same for 30 frames and nothing else moves, the scene
runs its scripts only twice a second until a result changes again. After three failures in a row
the scripts of the scene stop and the output reports the error.

```js
import { engine, props, audio, input, scene, timeline } from "wprs";

const moon = scene.layer("moon");

export function update({ time }) {
    moon.rotation = time * 10;
    moon.opacity = 0.5 + audio.bass / 2;
    if (input.cursor.inside) moon.position = [input.cursor.x * 1920, input.cursor.y * 1080];
    if (audio.beat) timeline("flash").play();
    return props.get("speed") * time;
}
```

`engine` has `time`, `dt` and `locale` (the language picked among the package's languages, as for
its texts, in lowercase, or `""` when it declares none), `props.get(id)` the user properties
(a chosen file or folder reads `"wpak-user://<id>"`, never its path, and `null` while none is
chosen), `props.label(id)` the label of the chosen option of a combo in that language,
`props.color(id)` a color as `{ r, g, b, a }` from 0 to 1, `props.pressed(id)` whether a button was
pressed since the last frame,
`audio` `bands(16 | 32 | 64, "left" | "right" | "average")`, `bass`, `mid`, `treble`, `level` and
`beat`, `input.cursor` `x`, `y`, `inside` and `buttons`. `scene.layer(id)` overrides `position`,
`scale`, `rotation`, `opacity`, `visible`, `color` and `text` for the next frame, and `timeline(id)`
has `play()`, `pause()`, `reverse()` and `seek(t)`; engines that run `timeline_events` also give it
the parts of the newer interface listed below (`speed`, `stop()`, `time`, `duration`, `playing` and
`on(name, handler)` for its events and `ended`), which first-version scripts may use too. `Math.random` follows `clock.seed`.
`engine.daytime` goes from 0 at local midnight to almost 1, `Date` uses the local time zone of the
computer (also after it changes, without reading any file), and `time.format(pattern, date)` writes the
date (`date` or now) with the tokens of `date` values in the language of `engine.locale`, such as
`time.format("EEEE d 'de' MMMM")`. In a
first-version package a scene whose scripts mention `audio` or `cursor` uses that input as if a value
were bound to it, so it still needs the user's grant; second-version packages use what their scripts
declare.

### Script declarations

Each entry of `scripts` may declare what its module needs from the engine, so that an engine without
it refuses the package instead of running the module wrongly, and so that its inputs are known
without reading its text. At most 64 modules (more than 16 name `scripts2`), each path once.

| Field | Meaning |
|-------|---------|
| `events` | the functions the module exports and the events it handles, each once: `init` (once, before the first frame, with the module's `params`), `update` (each frame; the only one older engines call), `destroy`, `resize` (the canvas or the screen changed), `properties` (the user changed settings), `settings` (language, frame rate or quality changed), `timelineEvent` (`{ timeline, name, t }` of timeline and clip events), the pointer events of `solid` layers and `hit` bones `click`, `down`, `up` (input `clicks`), `enter`, `leave` and `move` (input `pointer`), delivered with `scene.layer(id).on(event, handler)` with the layer, local and canvas positions and the touched bone, and `mediaStatus`, `mediaPlayback`, `mediaProperties`, `mediaThumbnail` and `mediaTimeline` (input `media`) |
| `uses` | the parts of the `wprs` module it uses, each once: `audio`, `pointer` and `clicks` (those inputs), `media` (the `media` object and its colors, input `media`), `timers` (`setTimeout` and `setInterval` on the scene clock, at most once a frame and never under 16 ms), `storage` (`storage.get`, `set`, `delete` and `clear` per output or package, 64 KiB of JSON kept on the device only, removed with the package; `script_storage`), `layers` (`scene.spawn(template, …)`, `destroy`, `scene.order` and `scene.layers()`, at most 256 dynamic layers; `script_layers`), `geometry` (`scene.mesh(pool)` with `create`, `update` and `destroy`, at most 1 MiB of vertices a frame; needs `pools`; `script_geometry`), `effects` (`layer.effect(index or name)` with `enabled`, `set` and `reset`), `particles` (`particles(layer)` with `play`, `pause`, `stop`, `emit(n)` and `instance`), `sounds` (`sound(id)` with `play`, `pause`, `stop`, `playing` and `volume`), `animations` (texture frames, video playback, mesh and model clips, bones, shapes and attachments of layers), `camera` (`scene.camera`, `camera3d`, `parallax` and `background`), `math` (the `wprs/math` module of vectors, matrices, `smoothstep`, `mix` and color conversions), `shortcuts` (`openShortcut(prop)`, only inside a click), `console` (`console.log` and the like, limited, shown by the editor) and `shared` (values shared between modules and relative `import` of other modules of the package) |
| `params` | at most 32 settings of the module `{ "type": "number" \| "bool" \| "text" \| "color" \| "vec2", "default", "min", "max" }` that values bound to it with `fn` and `params` fill in; ids `[a-z0-9_]{1,32}` |

Any event but `update`, any part but `audio`, `pointer` and `clicks`, and `params` name `scripts2`;
handling pointer events needs a `solid` layer or `hit` bones. A value `{ "script", "fn", "params" }`
asks the module for the exported function `fn` (an identifier) with the `params` it declares, so
one module serves many values. Effect uses may carry a `name` (`[a-z0-9_]{1,32}`, unique in their
layer) by which scripts find them (`scripts2`).

The newer interface (`scripts2`) also gives every module `engine.fps`, `engine.quality` (0 to 3),
`engine.language`, `engine.daytime`, `engine.canvas` and `engine.screen` (`[w, h]`, without names of
outputs or monitors), `engine.device` (`desktop` or `mobile`), `engine.mode` (`wallpaper`,
`lockscreen`, `screensaver`, `editor` or `preview`) and `engine.orientation`; `props.all()`;
`audio.buffers(16 | 32 | 64)`, arrays updated in place; `input.cursor.scene` (canvas pixels),
`input.cursor.screen` (output pixels) and `input.cursor.left`; reading the evaluated `position`,
`scale`, `rotation`, `opacity`, `visible`, `color`, `parallax`, `size` and `name` of a layer and
writing `skew`, `origin`, `parallax`, `parent`, `size`, `font` and `align`; `layer.world`,
`layer.children` and `layer.setParent(id, keep)`; and `timeline(id).speed` (−100 to 100; below zero
the timeline runs backwards like after `reverse()`, and one that plays `once` ends at its start), `stop()`, `time`,
`duration`, `playing` and the event `ended`. `clock.seed: null` gives `Math.random` a new seed on
every start. Scripts never see paths, device names, the battery or the list of fonts.

`templates` maps at most 64 ids to layers that scripts with `layers` copy into the scene, validated
like the other layers (`script_layers`); a template id may not be the id of a layer. `pools` maps at
most 16 ids to `{ "type": "mesh", "max" (1 to 256 shapes), "max_vertices" (3 to 65536), "texture" }`,
the shapes scripts with `geometry` may build (`script_geometry`).

```json
"scripts": [{ "path": "scripts/bars.js", "events": ["init", "update", "click"], "uses": ["audio", "layers"],
              "params": { "speed": { "type": "number", "default": 1, "min": 0, "max": 4 } } }],
"templates": { "bar": { "id": "bar", "type": "solid", "color": "#ffffff", "size": [8, 100] } }
```

## Rebuilding a scene by hand

An author who rebuilds a wallpaper made with another program writes its parts with these fields;
the scene format has a field for each of them, and the features above say which ones an engine
draws today.

| The original has | In `scene.json` |
|------------------|-----------------|
| layers placed by their center with Y up from the bottom left, angles in counterclockwise radians and opacity from 0 to 100 | `transform.position` with `origin: [0.5, 0.5]`, y = canvas height − y, degrees = −radians × 180 / π, `opacity` / 100 |
| a scene of one picture without effects | a package of `kind: image` |
| a full-screen layer that applies effects to what is below it | an `adjustment` layer; with a region and its own transform, `size` and `redraw` |
| a flat color layer | a `solid` layer |
| an orthographic projection of a given size and the user's alignment | `canvas.size` and `canvas.fit` (`fill` covers the screen) |
| a parent that moves a layer without grouping it | `parent` and `inherit` |
| a layer color and a brightness above 1 | `color` and `brightness` (with `canvas.hdr`, so that light above white is kept) |
| a texture without mipmaps, with sharp pixels or repeated outside its edges | the asset's `mipmaps`, `filter: nearest` and `wrap` |
| a sprite sheet or an animated GIF that plays by itself | the asset's `atlas` or `rects` with `fps` or `durations`, or `animated: true` |
| a text with a maximum width and number of rows, vertical alignment or an opaque background | `wrap` (`width`, `lines`, `ellipsis`), `valign`, `line_height`, `background` and `padding` of the text |
| a font collection | the `.ttc` asset with `face` |
| a clock or a date written by a script | `{ "clock" }` and `{ "date" }` values, which wake the scene only when the text changes |
| the time of day used as a number | `{ "clock": "day" }`, or `engine.daytime` in scripts |
| keyframes with Bézier handles, or each axis animated on its own | keys with `{ "in", "out" }` and `{ "x", "y" }` or `{ "r", "g", "b", "a" }` parts |
| an animation added to the value it starts from | `base` of the value |
| camera parallax with an amount, a delay and a share for the pointer, and a depth per axis | `canvas.parallax` and `parallax: [x, y]` of the layers |
| the blending of a material or of particles | the layer's `blend` |
| colors mixed as encoded values | `canvas.blend_space: srgb` |
| an effect mask painted in an editor | the effect's `mask`, a `linear` picture |
| several properties moved by one animation | the same `timeline` on each value |
| an animation that counts frames | `frames` and `fps` of the timeline |
| a looping animation that blends its end into its start | `wrap: true` on a `repeat` timeline |
| named moments of an animation that show layers, play sounds or start other animations | `events` of the timeline, and `trigger` or `play_on` of `event:<name>`; scripts get them through `timelineEvent` |
| a script that changes the rate of an animation, stops it or reads the frame it is on | `timeline(id).speed`, `stop()`, `time` and `duration` (a frame is `time` × `fps`), and `on("ended", …)` |
| a sound object with several files played in turn or at random, with silences between them | `sounds` with `mode` and `pause` |
| background music, a whole song or MP3 files | `.mp3` assets, and `stream: true` for anything longer than 60 s |
| a sound played by an animation event | `play_on: "event:<name>"` |
| a text measured in points | `size` in pixels, points × 4 / 3 |
| a texture chosen with a checkbox or a list | `{ "prop", "map" }` of a `bool` or `combo` property |
| a texture the user replaces | a `file` property with `default_asset` |
| shader variants chosen before compiling | `options` of a package effect |
| shader uniforms shown as controls | `params` of a package effect |
| per-instance overrides of a particle system | `instance` of the `particles` layer, which may be bound to sound or the pointer |
| changes over the life of a particle | `render.size_over_life`, `color_over_life` and `init.alpha` |
| a deformable picture with bones | a `mesh` layer with a `.glb` exported from a rigging tool |
| a 3D model in FBX or OBJ | a `model` layer with the model converted to `.glb` |
| a wallpaper that runs a program | a `web` package exported with WebGL or WebAssembly; packages never run programs |

Shaders are written in WGSL. HLSL and GLSL code ports with these equivalences: `mul(a, b)` → `b * a`
for column vectors, `frac` → `fract`, `saturate(x)` → `clamp(x, 0.0, 1.0)`, `lerp` → `mix`,
`fmod(a, b)` → `a - b * trunc(a / b)`, `ddx` and `ddy` (or `dFdx` and `dFdy`) → `dpdx` and `dpdy`,
`float2` or `vec2` → `vec2<f32>`, a texture read → `sample(t, uv)` or `tap(uv)`, `#define` constants
→ `const` or an `override` named by `options`, and the time, time of day, pointer and sound globals →
`frame.time`, `daytime()`, `frame.cursor` and `band_left(i)` or `band_right(i)`. Package shaders
declare no bindings of their own: the input, the mask and up to eight more textures come through
`inputs` and `params`.
