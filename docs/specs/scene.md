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
  quality level, `seed` feeds procedural effects.
- `assets` maps at most 256 ids (`[a-z0-9_]{1,32}`) to `{ "path", "linear", "atlas" }`. Images
  (PNG, JPEG, WebP, GIF, BMP), fonts (TTF, OTF), videos (WebM, MP4, MKV, stored uncompressed),
  sounds (Ogg Vorbis, Opus, FLAC or WAV, see "Sound") and color lookup tables (`.cube`,
  `LUT_3D_SIZE` 2 to 65, domain 0 to 1, at most 16 MiB each and 64 MiB of table data per scene; a
  file named by several ids is read once) are used.
  `linear` marks data images such as depth or flow maps, which are read without color decoding.
  `scene.json` is at most 8 MiB and the images of a scene at most 512 MiB once decoded.

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
| `video` | `video` (asset id), `size`; plays in a loop |
| `text` | `text`, `font` (asset id or system family), `size`, `color`, `align` |
| `shape` | `shape` (`rect` with `radius`, `ellipse`, `ring`, `polygon`, `star`, `line`), `fill`, `gradient` (`radial`, `from`, `to`, 2 to 4 `stops`), `stroke` (`color`, `width`) |
| `spectrum` | `size`, `style` (`bars`, `circle`, `wave`), `bands` (16, 32 or 64), `channel`, `color`, `gap` |
| `solid` | `color`, `size` (the canvas by default) |
| `particles` | `system`, a key of `particles` (see below) |
| `sound` | `sound` (asset id), `loop`, `play`, `volume`, `pan`, `fade_in`, `fade_out` (see "Sound"); no visual field |
| `adjustment` | only `effects`, applied to everything below it |

`blend` is `normal`, `add`, `multiply`, `screen`, `lighten`, `darken`, `overlay`, `soft_light`,
`hard_light`, `difference`, `exclusion`, `color_dodge`, `color_burn`, `hue`, `saturation`,
`color` or `luminosity`. Composition uses premultiplied alpha. `mask` is an asset id or
`{ "texture" | "layer" | "shape", "channel", "invert", "feather" }`: textures and layers mask by
luminance or alpha (`r`, `g`, `b`, `a`, `luminance`), shapes by their outline. A layer used as a
mask (or a group holding one) may set `visible: false`: it is then not drawn but still masks. At
most 256 layers nested at most 8 levels; masks may not refer to each other in a cycle.

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
| Script | `{ "script": "scripts/spin.js" }` |

Non-literal values accept `scale`, `offset`, `clamp: [min, max]`, `invert` and `smooth` (seconds).
Easings are `linear`, `step`, `hold`, `in_`, `out_` or `in_out_` followed by `sine`, `quad`,
`cubic`, `quart`, `quint`, `expo`, `circ`, `back`, `elastic` or `bounce`, or a cubic Bézier
`[x1, y1, x2, y2]`. Colors interpolate per channel.

`timelines` (at most 64) have `id`, `duration`, `loop` (`once`, `repeat`, `pingpong`), `speed`,
`delay`, `autoplay` and `trigger` (`start`, `beat` or `prop:<id>`). The scene
clock only runs while the output is shown and active, keeps real time while a still scene waits
(up to ten minutes between frames, in steps of at most 0.1 s), and never catches up after a pause.

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

## Sound

```json
{ "id": "gusts", "type": "sound", "sound": "wind", "loop": true, "fade_in": 3, "fade_out": 3,
  "play": { "prop": "wind", "map": { "calm": false, "breeze": true } },
  "volume": { "timeline": "storm", "keys": [[0, 0.4], [20, 1, "in_out_sine"], [40, 0.4]] }, "pan": -0.3 }
```

A `sound` layer plays a sound asset: `.ogg` (Vorbis or Opus), `.opus`, `.flac` or `.wav`, mono or
stereo, sampled at 8 to 192 kHz, at most 8 MiB and 60 s long, and at most 180 s for all the sounds
of a scene together; the package reader checks the headers and the daemon stops decoding a sound
that goes on longer than its header says. A scene has at most 8 sound layers, which mix together.

| Field | Meaning |
|-------|---------|
| `loop` | `true` (default) repeats the sound without a gap; `false` plays it once each time `play` turns on |
| `play` | starts the sound when it turns on and stops it when it turns off (default `true`); a sound starts from its beginning |
| `volume` | `0` to `1` (default `1`) |
| `pan` | `-1` (left) to `1` (right), default `0` |
| `fade_in`, `fade_out` | seconds (`0` to `60`) the sound takes to rise when it starts and to fall when it stops |

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

## User properties and presets

User properties are declared in the manifest, under `properties` (id → definition), and scenes
refer to them by id. Each has a `type` and `label`, `help`, `group`, `visible_if` and `order`:

| `type` | Fields | Value |
|--------|--------|-------|
| `color` | `default`, `alpha` | `#rrggbb` or `#rrggbbaa` |
| `slider` | `min`, `max`, `step`, `default`, `integer`, `unit` | number, clamped to the range |
| `bool` | `default` | `true` or `false` |
| `combo` | `options` (strings or `{ "value", "label" }`), `default` | one of the options |
| `text` | `default`, `max_len` (≤ 1024), `multiline` | text |
| `file` | `accept` | absolute path of an image chosen by the user, or `null` |
| `group`, `label` | | presentation only |

`presets` is a list of `{ "id", "label", "values" }`; a preset cannot set a `file` property. Values
are changed through `GetSceneProperties`, `SetSceneProperty`, `ApplyScenePreset` and
`ResetSceneProperties` (`dbus.md`) or `wprs-ctl prop`, and stored in `config.toml`. A stored value
whose property no longer exists, changed its type or falls outside its range after an update of the
package is ignored for the default, with a warning in the log, in `GetSceneProperties` (`warnings`)
and in `wprs-ctl prop ls`. Labels and help are text or `{ "<language>": text }`, and in
`wallpaper-rs.wpak/2` also `"@key"` of the manifest's `strings`; ids and limits depend on the
manifest version (`package.md`). `group` names a property of type `group`, under which the property
is listed; `visible_if` (`{ "<other id>": value }`) hides it while another property has a different
value; `order` sorts properties and groups. The app shows these forms under *Scene settings* for
each screen on its Settings page. The scenes in the `scenes/` folder of the repository are examples
of this format, dedicated to the public domain.

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
(`package.md`).

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
its texts, in lowercase, or `""` when it declares none), `props.get(id)` the user properties,
`audio` `bands(16 | 32 | 64, "left" | "right" | "average")`, `bass`, `mid`, `treble`, `level` and
`beat`, `input.cursor` `x`, `y`, `inside` and `buttons`. `scene.layer(id)` overrides `position`,
`scale`, `rotation`, `opacity`, `visible`, `color` and `text` for the next frame, and `timeline(id)`
has `play()`, `pause()`, `reverse()` and `seek(t)`. `Math.random` follows `clock.seed`. A scene
whose scripts mention `audio` or `cursor` uses that input as if a value were bound to it, so it
still needs the user's grant.
