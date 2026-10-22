# Scene format

A scene package is a `.wpak` package (see `repo.md` for repositories) whose manifest has
`"kind": "scene"` and `"entry": "scene.json"`. The scene lists its assets by package path; every
path must be in the package. `wprs-ctl pack build` and every reader check the scene: unknown
fields, broken references and out-of-range values are refused with the JSON path of the field,
such as `/layers/2/effects/0/params/radius is 300, expected 0..=128`. The model lives in
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
- `clock.max_fps` caps the frame rate (`1..=240`, default 60), `poster_time` is the instant shown
  where motion is not wanted, `seed` feeds procedural effects.
- `assets` maps at most 256 ids (`[a-z0-9_]{1,32}`) to `{ "path", "linear", "atlas" }`. Images
  (PNG, JPEG, WebP, GIF, BMP), fonts (TTF, OTF) and videos (WebM, MP4, MKV, stored uncompressed)
  are used. `scene.json` is at most 8 MiB and the images of a scene at most 512 MiB once decoded.

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
| `particles` | `system`, a key of `particles` (emitters, initial values, forces, colliders); not drawn yet |
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

An effect use is `{ "effect", "enabled", "mask", "params", "optional" }`. Built-in effects:

| `effect` | Parameters | Passes |
|----------|------------|--------|
| `blur` | `radius` (0–128 px) | separable Gaussian |
| `color_adjust` | `exposure` (EV), `contrast`, `saturation`, `hue` (degrees, OKLab) | 1 |
| `waves` | `amplitude` (px), `frequency`, `speed`, `direction` (degrees) | 1, animated |
| `bloom` | `threshold`, `intensity`, `radius` (px), `knee` | bright pass, blur and add at half size |

Each effect is a JSON descriptor (`id`, `uses`, `mask_mode`, `params`, `passes`, `cost`, `fuse`,
`reach`) and WGSL fragment entry points that share one prelude: group 0 holds the frame uniform
(time, size, pointer, audio levels and 64 bands) and the sampler, group 1 the parameters as 16
`vec4` slots in declaration order, the input, the mask and up to four more textures (`previous`,
`input`, `source`, `pass:N`). Built-in descriptors are in `crates/wprs-scene/src/effects.json`
and their passes in `effects.wgsl`; a new effect is a descriptor plus its entry points there.
`mask_mode: mix` blends the result with the input by the mask; `strength` lets the effect read
`mask_at(uv)` itself. The mask of an effect is a texture; layer and shape masks are refused there
and belong on the layer. With a lower quality level `optional` effects are dropped first.

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

`presets` is a list of `{ "id", "label", "values" }`; a preset cannot set a `file` property.
Values are changed through `GetSceneProperties`, `SetSceneProperty`, `ApplyScenePreset` and
`ResetSceneProperties` (`dbus.md`) or `wprs-ctl prop`, and stored in `config.toml`.

## Inputs

Audio analysis, the pointer and script outputs reach the renderer as plain structures of
`wprs-scene`: `AudioFrame` (64 bands per channel and their average, `bass`, `mid`, `treble`,
`level`, `beat`, `silent`; all zero while audio is off), `PointerState` (position, smoothed
position, velocity, buttons, `inside`, in fractions of the screen) and `ScriptOutputs` (values of
script sources, layer overrides and timeline commands). A scene only renders again for an input it
uses. The pointer follows the reported position with a critically damped spring that settles in
about 0.15 s and returns to the center when the pointer leaves the screen. Audio and the pointer
need the user's grant for the scene (`dbus.md`, `Inputs1`).

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

`engine` has `time` and `dt`, `props.get(id)` the user properties, `audio` `bands(16 | 32 | 64,
"left" | "right" | "average")`, `bass`, `mid`, `treble`, `level` and `beat`, `input.cursor` `x`, `y`,
`inside` and `buttons`. `scene.layer(id)` overrides `position`, `scale`, `rotation`, `opacity`,
`visible`, `color` and `text` for the next frame, and `timeline(id)` has `play()`, `pause()`,
`reverse()` and `seek(t)`. `Math.random` follows `clock.seed`. A scene whose scripts mention `audio`
or `cursor` uses that input as if a value were bound to it, so it still needs the user's grant.
