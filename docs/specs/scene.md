# Scene format

A scene package is a `.wpak` package (see `package.md` for packages and their manifest, `repo.md`
for repositories) whose manifest has `"kind": "scene"` and `"entry": "scene.json"`. The scene
lists its assets by package path; every path must be in the package. `wprs-ctl pack build` and
every reader check the scene: unknown fields, broken references and out-of-range values are refused
with the JSON path of the field, such as `/layers/2/effects/0/params/radius is 300, expected
0..=128`. The model lives in
`crates/wprs-scene`; `wprs-ctl scene bench <file.wpak>` renders a package offscreen and reports
what each frame costs, how long the scene takes to load and draw its first frame, and the GPU and
main memory it holds (`--screens 2` shows it on two screens at once).

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
| `parallax` | `{ "amount" (0 to 4, default 1), "delay" (0 to 2 s, default 0.15), "pointer" (0 to 1, default 1), "gyro" }`: how far and how softly layers with a `parallax` follow the pointer, how much the pointer counts and whether the tilt of the phone moves them too (on Android, through the same motion sensors as picture parallax, only while the scene is shown; the phone has no pointer, so there every scene with parallax layers follows the tilt as if `gyro` were on, or the launcher pages when the phone has no motion sensor, and neither moves them with the battery saver on); `pointer: 0` needs no pointer; a layer moves by its `parallax` times the canvas size times `amount` times the offset of the pointer from the middle (−0.5 to 0.5) times `pointer`, plus half the tilt; where the system asks for reduced motion, layers stay in place and neither the pointer nor the tilt moves them | `parallax2` |
| `camera` | `{ "center", "zoom" (0.1 to 10), "rotation", "shake" }` moves, zooms and turns the whole scene as one transform without extra passes: the point `center` of the canvas (its middle by default) shows at the middle, `zoom` times larger and turned clockwise by `rotation` degrees; `shake` is `{ "enabled", "amplitude" (0 to 1000 px), "roughness" (0 to 1), "speed" (0 to 60 Hz) }`, a smooth noise of the scene time (four octaves, each weighed by `roughness` against the one before) that moves the scene by at most `amplitude` pixels and asks for a frame every frame while it runs; it is turned off where motion is reduced and at the lowest quality level, and the scene then stands still; all of them may be bound | `camera` |
| `bloom` | `{ "threshold" (0 to 4), "strength" (0 to 8), "radius" (0 to 128 px), "tint", "hdr": { "threshold" (0 to 16), "strength" (0 to 8) } }`, a glow of the bright parts drawn after everything else, with the `bloom` effect and its `tint` over the whole canvas, as an adjustment layer at the top would; the `hdr` threshold and strength replace the plain ones at level `q3`, and at `q2` on mains power; it costs like that effect (four passes at half size), is dropped where the effect is and is drawn only when something below it changed | `bloom` |
| `hdr`, `tonemap` | `hdr: true` keeps colors brighter than white in 16-bit intermediate images, and `tonemap` (`none`, `aces` or `agx`, only with `hdr`) maps them to the screen | `hdr` |
| `clear` | `false` draws every frame over the previous one, which leaves trails: the canvas is filled with `background` only when the view starts or changes size, and no layer is kept as a still picture; each frame that changes is drawn whole instead of only the part that changed, so it costs more | `keep_frame` |
| `perspective` | distance of the viewer in pixels (100 to 100000) for layers tilted with `transform.tilt` and moved with `transform.z`; without it the distance at which the canvas height fills the `fov` of `camera3d`, or 45° | `tilt` |
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
it loops and the frame (`layer_playback`). A sheet without `fps` or `durations` shows its first frame. The scene asks for a frame only when the next frame of a shown
sheet is due, never while the layer is hidden, and the lowest quality level shows the first frame of
the range without moving.

Each picture is decoded once and uploaded once per GPU, and every screen that shows the scene
draws from that one texture. Once a picture is on the GPU its decoded copy leaves main memory; the
engine reads it again from the package only when no screen holds the texture any more. Every
shader and pipeline a scene needs is built when the scene loads, never during a frame, and a frame
hands the textures of a finished layer to the next layer that needs one of the same size, so a
steady frame creates nothing and makes no allocations of its own. Evaluating a frame allocates
nothing either: `Runtime::frame_in` takes the lists of the previous frame back (`Frame::spare`).

Each video asset has one decoder, shared by every layer, mask and effect that shows it on every
screen. The first layer of the scene that shows the asset, as a `video` layer or as the picture of
an `image` layer, steers it with its `playback`: `rate` (a video plays from 0.1 to 4 times, and its
time, its end and what scripts read follow the rate it plays at; 0 holds the video), `playing`,
`loop` (on by default), `start` and `end` in seconds. The time of a video runs on the scene clock
while it plays and something shows it: a video that no visible layer, mask or effect shows waits
where it is and its decoder pauses, as for a paused or finished video, and none of them asks for
frames. Without `loop` the video stops at `end`, or at its own end (read from the header of its
WebM, MKV or MP4 file), and tells scripts. On battery its decoder runs no faster than the frame rate
of the scene, which `clock.max_fps` caps. On the phone the decoder is the platform's MediaCodec,
through the system player, which plays only the part between `start` and `end`.

Fonts may also be TrueType or OpenType collections (`.ttc`, `.otc`) and compressed `.woff2` files
(`fonts2`). Videos may be used as masks, effect textures and the picture of a spot light
(`video_textures`), and wherever a picture is; sounds may also be MP3 (see "Sound"). KTX2 textures are accepted by the package
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

## Cost and frame rate

The engine estimates what each frame of a scene costs before it draws it, without the GPU. A unit
is one full-screen pass that reads one texel per pixel. Each layer adds the units of its effects
(taps × passes × scale², at the size the quality level gives them), one unit when it is drawn
apart (effects, masks, `cache`, blend modes that are not `normal`, `add`, `multiply` or `screen`)
and two more when its blend mode copies what lies below it, all multiplied by the share of the
canvas the layer is drawn over. That share is the area the effects actually cover, clipped to the
canvas: an adjustment layer covers its whole target (the canvas, or its group, which an adjustment
layer widens to the canvas) whatever `size` it declares; a group covers the bounding box of its
layers, not the sum of their areas; any other layer covers the box around its `size` (an image or
video without one, text, particles and lights count as the whole canvas) moved, turned, skewed and
scaled by fixed transforms of its own and its groups, grown by the `reach` of its effects. A layer
whose place can change while it plays (a bound or animated `position`, `rotation`, `skew` or
`scale`, a `tilt`, an `anchor`, `parallax`, a `parent`, an `attach`, a bound `reach`, or any script
in the scene) counts as the whole canvas. A
layer that masks or feeds others adds one more unit times its share, once however many read it,
since it is drawn on its own at most once a frame. Particles add their overdraw, `canvas.bloom` adds the units of its effect over the whole canvas, a
`canvas.camera` makes every layer count as the whole canvas, and every frame costs 1.5 units for composing and copying. A scene
that costs more at level `q2` than the 8 units of older integrated and phone GPUs gets a warning
from `wprs-ctl pack check`, `pack build` and the scene editor, since those devices show it with
fewer details. The quality level is the highest whose cost fits the budget of the device: 4, 8, 16
or 40 units for the tiers of software rendering or GPUs without compute shaders, older integrated
and phone GPUs (Intel HD and UHD, AMD Vega and other older APUs, Mali-G up to G710, Adreno 5xx and
6xx, or less than 4 GiB of memory), newer integrated GPUs and discrete GPUs. A scene over budget at the lowest
level stays at `poster_time`.

The tier starts from the model of the graphics card, as above. The first time a program renders
scenes with a graphics card and driver, it times a short render: frames of 16 passes over a
1920×1080 target, each reading a texture of that size, for about 300 ms, and the time of a unit is
taken from the fastest quarter of the frames. A unit of at most 0.2 ms gives the tier of discrete
GPUs, 0.5 ms that of newer integrated GPUs, 2 ms that of older integrated and phone GPUs, and slower
ones the lowest tier; the measure moves the tier of the model by one step at most, up or down, and
less than 4 GiB of memory or textures smaller than 8192 pixels still cap it.
Software rendering and GPUs without compute shaders keep the lowest tier without a measure. The
result is kept in `gpu-tier.json` in the state folder (`$XDG_STATE_HOME/wallpaper-rs/`, shared by
the daemon and `wprs-ctl scene bench`) or in the files of the phone, with the name, ids and backend
of the adapter and the version of its driver, and it is measured again when any of them changes.
A measure that lowered the tier, which a busy start can cause, is taken again on the next starts,
three times in all, and the fastest one counts (`runs` in the file).
`GetState().tier` (`"t0"` to `"t3"`) and `tier_measured` tell the result, and the phone lists it
among its diagnostics.

While a scene plays, the GPU time of its frames (measured as described below) moves the quality
level: when their average (an exponential average over the measured frames, started again after
more than a second without one) stays above the GPU budget for 2 seconds, the level goes one step
down; when the average, times the factor by which the level above was dearer (the average just
before the step down divided by the one 5 seconds after it, between 1 and 16, and at most one and a
half times the ratio between the estimated costs of the two levels, so a burst of load that ends
right after the step down does not count), stays under half of the budget of the level above for 15
seconds, one step back up. When only that factor holds the level down, the level above is tried again
after a minute. A level that has to come back down within 30 seconds of a raise doubles both waits,
up to four times, and every raise that holds for 30 seconds halves them again. The level never goes
above the one that the tier, the cost, the power source and the temperature allow, and never rises on battery,
with power saving or while the GPU or CPU is warm (on phones, only while charging and cool). The
budget of a level is half of its frame interval on mains power and a quarter on battery, with power
saving and on phones; since a lower level may also draw fewer frames a second, its budget is larger,
and comparing the cost measured at the level above keeps a scene from bouncing between two levels.
Two changes are at least 5 seconds apart. The quality only moves on devices that time their frames,
which all do, with timestamps or by waiting (see below). `GetState().outputs[x].quality` and
`quality_reason` tell the level and why it is below the highest; the app, the tray and the phone
show both, translated.

Every view of a scene also holds a hard budget, whatever the quality level: each effect use, built
in or packaged, does its units (at the highest level, four times for an effect with a `state`, which
may run four steps a frame) times the share it covers times the megapixels drawn, at least half a
unit, and the uses together may do at most 2 000, 4 000, 8 000 or 16 000 of those units a frame on
the four tiers; at most 16 uses with a `state` may run in a scene. The engine adds the uses in the
order of the layers, a group before its layers, and leaves out each use that does not fit with
`the effect <id> on <layer> was left out: the effects of the scene would do more than <limit> units
of work a frame on this GPU` (or `... the scene has more than 16 effects that keep a state`). When
the scene loads, before it builds anything, it leaves out only the uses that cannot fit at any size
(past the 16 stateful uses, or past the limit at half a unit each), so a package effect whose uses
are all left out is never built; it counts the work whenever the size a view draws at changes, at
that size and over the part of the canvas the screen shows, so a large canvas shown on a small
output keeps the effects that fit there. The state textures of all uses together may hold at most
512 MiB, beside the 512 MiB of textures a frame may borrow; a use whose state would go past it is
left out with `its state textures would take the scene past 512 MiB`. A frame may borrow at most
512 MiB of textures, or four full frames of the output if that is more, and a view holds at most
1 GiB of textures in all, or its states, its cached layers and its target plus the frame's share
and two full frames more if that is more (so an 8K output keeps its effects): before it makes
another texture it drops the ones it used longest ago, then the cached layers this frame has not
drawn, and a use whose texture still does not fit is left out with `its textures would take more
than the <n> MiB of textures this view may hold` (or `... a frame may borrow`). Layers draw into textures
whose sides grow in steps of an eighth of a power of two (at least 64 pixels), so a zooming layer
reuses a few sizes. The first pass that writes a target of a package effect clears
it as it draws instead of in a pass of its own, and an adjustment layer reads what is below it
directly instead of a copy. `wprs-ctl scene bench` prints these notes after its
numbers.

On battery the level drops one step and the rate is capped at 30 frames per second (15 with
software rendering); power saving (the `power-saver` profile on Linux, battery saver on Android)
drops one step more. A GPU or CPU within 20 °C of its critical temperature (95 °C when its driver
reports none) drops one level and caps the rate at 30; within 10 °C, or once the system starts
throttling it, it falls to the lowest level and 15 frames per second. Other sensors, such as
drives, memory or network cards, do not count, and GPUs whose driver exposes no sensor are not read. Where the system asks for
reduced motion, the scene stays at `poster_time`: parallax, camera shake and timelines stop, and
the frame is drawn again only once a minute, so that clocks and dates stay right.

Each frame redraws only what changed: the layers that moved, appeared, disappeared or changed,
both where they were and where they are, plus adjustment layers with effects that read around a
pixel when a change reaches them. When that covers less than half of the screen, the rest of the
previous frame is kept; when nothing changed, nothing is drawn, and on the desktop the frame is not
composed onto the screen or sent to a stream either, unless a stream would otherwise go without
frames. Layer masks that use other layers redraw everything. Particles count as changed only when
their simulation took a step, so a held scene or particles at rest leave the damage to what moves,
and a view whose memory was freed while the scene stood still stays clean until something changes,
when it redraws everything.

On the desktop the damage of the scene, grown by a pixel for filtering and scaled to the output,
also limits what is composed and sent. A PipeWire stream in shared memory composes and reads back
only that rectangle and copies into each buffer the rectangles of every frame it missed (a buffer
older than the last eight frames, or one that was never drawn, gets everything); a stream in GPU
memory draws into each buffer only what changed since that buffer was last drawn, keeping the rest
of its pixels between frames. Both kinds of buffer carry `SPA_META_VideoDamage` with the rectangle
that changed since the previous buffer sent (a resent frame carries none), for consumers that read
it. Transitions, parallax and framed scenes compose the whole frame. Screens drawn through the
graphics API (Wayland and X11 surfaces, the phone) still compose every frame whole, because the
API exposes neither the age of the buffer it hands out nor a partial present; the scene behind
them keeps redrawing only what changed.

Where the user turns on `playback.flash_guard`, the engine measures the brightness of each frame on
a 32 × 18 grid. A flash is a pair of opposite changes of at least 10 % of the full brightness, the
darker side below 80 %, in at least a thirty-sixth of the grid at once (about a quarter of the
central field of view); when a scene flashes more than three times a second, it plays slower until
it flashes at most three times, down to a quarter of its speed, and if it still flashes faster it
stops at its current frame. The guard measures every frame that changed, reading the grid back from
the GPU one frame later so that drawing never waits for it; it is off by default.

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
B and S swapped), `phoenix` min(B, S) − max(B, S) + 1, `average` (B + S) / 2, `negation`
1 − |1 − B − S|, `tint` min(1, B × S / lum(S)), where lum is 0.3 R + 0.59 G + 0.11 B, so that the
layer colors the backdrop without darkening it, and `diffuse_light` min(1, 2 B × S), the layer as a
light falling on the backdrop, neutral at one half. `tint` and `diffuse_light` are approximations
of these looks: their formulas may still change to match reference pictures, and scenes that need
an exact result should use the other modes.

| Layer field | Meaning | Feature |
|-------------|---------|---------|
| `brightness` | 0 to 16 (default 1), multiplies the color of the layer; above 1 it only shows with `canvas.hdr` | `layer_color` |
| `color` | on `image`, `video`, `particles` and `group` layers, a color that multiplies the layer (white by default); a group applies it to its children together | `layer_color` |
| `parent` | the id of another drawn layer whose transform moves, turns and scales this one without changing the drawing order; the position of the layer is then relative to the position of its parent, in its turned and scaled frame (at the parent's position by default), instead of to its group; chains go at most 8 deep and never back to themselves | `parent` |
| `inherit` | what follows the parent: `transform`, `visible` and `opacity` (default `transform` and `visible`), only with `parent` | `parent` |
| `transform.tilt` | `[x, y]` degrees (−89 to 89) of tilt in depth around the position of the layer, seen with `canvas.perspective`: a positive `x` turns the bottom edge towards the viewer, a positive `y` the left edge | `tilt` |
| `transform.z` | pixels towards the viewer (−100000 to 100000); a layer at or behind the viewer is not drawn | `tilt` |
| `parallax` | a number or `[x, y]` (−10 to 10 each), with a different depth per axis | `parallax2` when the two differ |
| `lighting` | on `image`, `video`, `text` and `particles` layers, `{ "normal", "roughness", "reflection", "emissive", "strength", "env" }`: a `linear` normal map, roughness and reflection from 0 to 1, light of its own from 0 to 16, how much the lights count from 0 to 1 and a picture to reflect, how the layer takes the light of `light` layers | `lighting2d` |
| `playback` | on `image` layers showing moving pictures or videos and on `video` layers, `{ "rate" (0 to 16; a video plays at 0 or from 0.1 to 4), "playing", "loop", "start", "end" (seconds), "frame" }`; `frame` picks a frame of a sprite sheet and may be bound to keyframes, the pointer, the audio or a script | `layer_playback` |
| mask `strength` | 0 to 1 (default 1), weakens the mask: m′ = mix(1, m, strength) | `mask_strength` |
| adjustment `size` | `[width, height]` in pixels (at most twice the canvas): only that rectangle around the layer position (placed by `origin`, without the rotation, skew or scale of the layer) is captured and its effects change only that part, and `redraw: true` draws what was captured again with the whole transform of the layer, its opacity, blend mode and mask, as a magnifier or a mirror; without `size` the whole canvas is captured and turned around its middle. A capture is one copy of the rectangle and one pass more, and any change in the scene redraws the whole frame | `capture` |
| adjustment `rgb` | `true` sends what the layer captures, shrunk to at most 64 × 16, to the RGB lighting of the computer, at most 30 times a second; the manifest must set `permissions.rgb` (`package.md`) | `rgb` |
| `attach` | `{ "layer", "bone" \| "point", "offset": [x, y], "inherit" }`: follows a bone or a node of a `mesh` or `model` layer instead of a `parent`, as posed in the same frame; `inherit` lists `position`, `rotation` and `scale` (all by default; without `position` the layer stays on the origin of the mesh layer); `offset` is in pixels along the bone; the layer hides with its mesh layer, and its own `transform` applies on top as under a parent; a layer may not follow itself, through other attachments either | `attach` |
| `solid` | `true` or `{ "alpha" (0 to 1) }`: the layer takes the pointer for `click:`, `enter:` and `leave:` triggers and script pointer events, over its box or, with `alpha`, only where it is at least that opaque (pictures by the alpha of their pixels, the other kinds by their color and opacity); a solid group takes the pointer over the boxes of its layers; layers are tested from the top down, as they were drawn in the last frame, with their transforms, parents and parallax, and hidden layers never take it | `layer_events` with `alpha` |
| `depth_test` | inside a group with `space: 3d`, a flat layer hides behind the models in front of it instead of covering them | `model3d` |

A layer with `tilt` or `z` is drawn as a projected quadrilateral: one 3 × 3 projective transform
instead of the flat one, so the GPU spreads its picture with the right perspective and it costs
nothing more. The projection follows the camera, so zooming the camera zooms a tilted layer as a
picture of it. The effects of a tilted layer run in its own plane before it is projected, and its
mask leans with it. A tilted group is drawn apart first (one more pass over its box) and then
projected, and its layers lean with it; tilts nested in groups add up. The pointer finds a tilted
layer where it shows.

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

Outlines, shadows and blurs are drawn from a distance field: the letters are drawn once, the GPU
spreads their edges into distances with a jump flood (a few passes over a texture of the text, made
once each time the text, its layout or the room of its styles changes), and the shader draws the
fill, the outline, the shadow and the blur from it, so a change of their colors, widths, offsets or
blurs changes only numbers. With `render: auto` a text with a style uses the field at a reference
size (twice its size, from 16 to 256 pixels), so it grows and shrinks without being drawn again;
`raster` draws it again at its size on the screen, as plain texts are, and `sdf` uses the field
even without a style. The texture keeps room around the letters for the widest style, rounded up to
a power of two, so animating a style within that room draws nothing again. At the lowest quality
level shadows blur at most 4 pixels. Devices whose GPU cannot draw into 16-bit float textures draw
styled texts as plain ones.

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
into limbs. A scene has at most 16 `mesh` and `model` layers. The editor cuts such a mesh out of a
picture of the scene, along a grid or its outline, and places its bones (see `editor.md`).

Positions are in pixels from the middle of the box the layer fills, x to the right and y up as glTF
writes them, and UVs start at the top left of the picture. Every node of the file is a joint: the
bones of skins, and the nodes that carry a mesh without a skin, which then move as one piece. The
rest pose is the transform of each node; vertices follow at most 4 bones whose weights add up to 1
(within 0.02, then made exact). The engine skins the vertices on the GPU into a picture of the box
(half its pixels at the lowest quality and three quarters at the next) and draws that picture like
any other layer, with its transform, effects, masks and blend mode; it draws it again only when the
pose, the blend shapes, the order of the parts or a video texture changed, and a still or settled
skeleton asks for no frames. Parts moved out of the box are cut at its edge, so `size` leaves them
room.

Files are refused that have a bone cycle or a node with two parents, a vertex whose weights do not
add up to 1 or that follows a bone of no skin, an index out of its vertices, a value that is not a
number, sparse accessors, primitives other than triangles (strips and fans are read as triangles),
animation keys out of order, more than 65 536 vertices, 256 bones and parts, 64 drawn primitives,
524 288 blend shape vertices or 4 194 304 animation values, or 128 MiB of meshes for all the mesh
layers of a scene; meshes are read whole, once, when the scene is.

| Field | Meaning |
|-------|---------|
| `mesh` | the `.glb` asset |
| `texture` | a picture or video asset (or bound value) that replaces the textures of the file; without it, the picture of the first material that has one, or white; the base color of each material multiplies its parts |
| `size`, `color` | the box in pixels the mesh fills (the canvas by default) and a color that multiplies it |
| `parts` | per part (mesh name of the file): `order` (−1000 to 1000, may follow keyframes with `step`), `clip_to` (another part it only shows over) and `visible`; parts are drawn by `order` (0 by default), in the order of the file when it is the same |
| `depth` | `{ "scale" (0 to 10, 1 by default), "cull" }`: how far the depth of the vertices (z of the file, towards the viewer, in pixels) pushes them with the perspective of `canvas.perspective` or of the 3D camera, also under `transform.tilt`, and whether triangles seen from behind (turning clockwise on screen) are hidden; without `depth` the depth of vertices is ignored |
| `clips` | at most 64 `{ "id", "animation", "from", "to", "loop", "events", "bones", "shapes", "order" }`: a stretch of an animation of the file (the first one without `animation`, unless the clip has keys of its own) in seconds within its length, played `repeat` (default), `pingpong` or `once`, with named `events` like timelines, and keys of its own on the same seconds (below) |
| `animations` | at most 16 animation layers `{ "clip", "weight" (0 to 1), "rate" (0 to 16), "additive", "blend_in", "blend_out" (0 to 10 s), "enabled", "bones" }`, mixed in order: replacing layers first, then additive ones; `bones` limits a layer to those bones; bones without a track keep their rest pose |
| `ik` | at most 16 chains `{ "chain" (2 to 8 bones, each hanging from the one before), "target", "limits": { "<bone>": [min, max] }, "weight", "bend": "clockwise" \| "counterclockwise" }`; `target` is a point in pixels of the box from its top left (which may be bound), `"cursor"`, `"bone:<name>"` (any node of the file) or `"layer:<id>"`; see "Reaching and swinging" (`ik`) |
| `physics` | at most 64 `{ "bone", "mode": "spring" \| "rigid" \| "rope", "stiffness", "damping", "inertia" (0 to 1, 0.5 by default), "wind" (0 to 1), "gravity" (−10 to 10), "rotation": [min, max] degrees, "translation": [min, max] px }`; see "Reaching and swinging" (`bone_physics`) |
| `rules` | at most 64 `{ "bone", "parent", "weight" }`: moves a bone over to another parent as `weight` goes from 0 to 1, keeping its pose, as a sword passing from a rock to a hand (`bone_rules`) |
| `shapes`, `expressions` | weights from 0 to 1 of the blend shapes of the file, and at most 64 expressions `{ "<name>": { "<shape>": weight (−1 to 1) } }` whose names `shapes` may also set; at most 8 shapes act at once on each part, the heaviest (`blend_shapes`) |
| `hit` | bones whose area takes the pointer for scripts, which then learn the bone that was touched (`layer_events`): the layer takes the pointer like a `solid` one, and each event carries the listed bone that bends the drawn triangle under the pointer, or the listed bone it hangs from, else `null` |

The keys of a clip are lists of `[time, value]` keyframes with an optional easing, as in timelines:

| Key | Meaning |
|-----|---------|
| `bones` | per bone (a node of the file): `rotation` in degrees clockwise, `position` `[x, y]` in pixels with y down in the space of its parent, and `scale` `[x, y]`, each added to its rest pose; they replace the tracks of the file for that bone |
| `shapes` | per blend shape: its weight (−1 to 1) |
| `order` | per part: the `order` it takes while the clip plays, rounded to whole numbers; `parts.<name>.order` still wins when set |

A replacing layer moves each bone from what the layers before it left towards its own pose by its
weight; rotations are blended along the shorter way and normalized. An additive layer adds what its
clip moves away from the rest pose, times its weight. The weight of a layer also follows its fade:
it grows over `blend_in` when the layer starts or is enabled and falls over `blend_out` when it is
disabled, stopped or, for `once`, when it ends, so two layers that swap `enabled` cross-fade. A layer
plays while it is shown and weighs more than nothing; a `once` clip then holds its last pose and
tells scripts that it ended. Blend shapes follow the same mix (the tracks of the file named `weights`
and the `shapes` of clips), and `shapes` and the expressions they name add to it. A `rules` entry
blends the place the bone would have under its own parent with the place it would have under the new
one, which keeps the place it had when the rule began to weigh (`bone_rules`); a rule may not close a
loop of parents. Cutouts use a stencil: a part with `clip_to` only shows where the part it follows
was drawn, as last drawn. The pose is worked out at most once per frame, only for shown layers, with
no allocation per frame; at the lowest quality moving skeletons ask for at most 30 frames a second.

```json
{ "id": "fox", "type": "mesh", "mesh": "fox", "size": [600, 900],
  "clips": [{ "id": "idle", "animation": "idle", "events": [{ "t": 1.2, "name": "blink" }] },
            { "id": "wave", "loop": "pingpong", "bones": { "arm": { "rotation": [[0, 0], [0.8, 35, "in_out_sine"]] } },
              "order": { "tail": [[0, 1]] } }],
  "animations": [{ "clip": "idle" }, { "clip": "wave", "bones": ["arm", "hand"], "blend_in": 0.3 }],
  "ik": [{ "chain": ["neck", "head"], "target": "cursor", "limits": { "head": [-30, 30] } }],
  "physics": [{ "bone": "tail", "mode": "rope", "wind": 0.6 }],
  "shapes": { "smile": { "prop": "mood" } } }
```

#### Reaching and swinging

Each frame the pose is worked out in this order: the animation layers, the `rules`, the `ik`
chains in their order, what scripts set (see "Bones, blend shapes and attachments"), and the
`physics`. Rotations of bones are measured in degrees clockwise from their rest pose, as in clips.

A chain turns its bones so that its end (where the first child of its last bone starts, or as far
again along the last bone when it has none) reaches the target. Chains of two bones are solved
exactly, bending at their middle joint the way `bend` says; longer chains, and chains whose `limits`
stop a bone, take at most 10 rounds that turn each bone towards the target from the end back. A
target out of reach stretches the chain towards it. `limits` keep each listed bone within `[min,
max]` degrees of its rest pose and `weight` blends the turns with the pose. `"cursor"` follows the
pointer only while the scene may read it (the pointer input, told to the user like any other use of
the pointer); without it the chain keeps its pose. `"layer:<id>"` follows the position of that layer.

Bodies of `physics` swing at a fixed step of 1/60 s (1/30 s on battery), at most 15 steps a frame,
so that they do not depend on the frame rate; longer gaps are dropped. A `spring` pulls the end of
its bone back to where the pose puts it (`stiffness`), a `rigid` bone swings like a pendulum held
loosely to its pose, and a `rope` links the bone and its first children (those without a body of
their own, at most 8 links) into a chain that hangs and sways. `inertia` is how much a body stays
behind when its parent moves, `damping` how fast it calms down, `gravity` pulls down (98 pixels per
second squared for each unit), `wind` is how much `canvas.wind` (in pixels a second, with gusts)
pushes it, `rotation` bounds how far each link turns from its pose and `translation` lets the head of
the bone itself move that many pixels each way. A body that has not moved for half a second, while
its pose and the wind stay still, asks for no frames; at the lowest quality bodies keep the pose
of the animation and ask for nothing. Scripts can push bones with `impulse`.

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
Audio values go from 0 to 1: each of the 64 bands per channel (40 Hz to 16 kHz, spaced evenly in
pitch) is its level in decibels above −70 dB, relative to the loudest band of the last seconds, so
that quiet and loud music both fill the range (`gain: "fixed"` measures it from full scale instead);
`bass`, `mid` and `treble` average the bands below 250 Hz, up to 4 kHz and above, `level` is the
loudness of the whole sound and `beat` is 1 on a beat. Values rise within 20 ms and fall within
200 ms unless the scene sets `audio`. The order is `bounds`, `exponent`, `invert`, `scale`,
`offset`, `clamp` and then `smooth`; `bounds` and `exponent` also shape properties, the pointer and
scripts, and an `exponent` keeps the sign of negative values.
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
| Layer as texture | `{ "layer": "<id>" }` for a `texture` or `mask` parameter of an effect: the layer is drawn on its own (even with `visible: false`) and fed to the effect aligned with the screen, so that each point of the effect reads the point of that layer above it, at the size the effect's layer has on the screen; it is drawn again only when that layer changes, and each one costs as much as a layer drawn on its own; layers may not feed themselves through other layers, masks or groups | `layer_textures` |

Timelines may name `events`, at most 64 `{ "t", "name" }` with `t` within the duration and names
of `[a-z0-9_]{1,32}`; each fires once per pass, also when a frame skips over it (on a `pingpong`
timeline once each way), in the order of their time. A `trigger` or a `play_on` of `event:<name>`
starts on it, as if it had started at the very moment of the event even when the frame came later,
so its own events up to that frame, those at 0 included, fire in the same frame, and scripts receive it (`timeline_events`, which every platform runs). A still scene wakes for the
next event only when a trigger, a sound or a script that the platform runs listens to its name. Clips of meshes and models
name events the same way.

Triggers may also be `click:<layer>`, `enter:<layer>` and `leave:<layer>` for a layer that is
`solid` or has `hit` bones, evaluated without scripts (`layer_events`; `click:` needs the `clicks`
input, the others `pointer`): a click is a press and a release of any button both on the layer,
which keeps the pointer from the press until the release (so dragging off it and letting go is no
click), and `enter`/`leave` follow the topmost solid layer under the pointer. They also start sounds
whose `play_on` names them. Pointer events never make extra frames: they are worked out when the
scene draws a frame for the pointer it already follows, and a still pointer makes none. And `media:play`, `media:pause`, `media:track` (a new track) or
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
`.cube` asset, both as literals, and a `texture` parameter may also take a layer (`{ "layer" }`,
see "Values"). The mask of an effect takes a texture, a layer or a shape, like the mask of a layer,
with `channel`, `invert`, `feather` and `strength`; a layer or shape mask (`effect_masks`) is drawn
over the box of the layer, so a shape is centered on it and sized in its pixels, and a layer masks
the effect where it lies on the screen. At lower quality levels (on battery, with software
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
over time with an `fps` above zero. `blur_motion` streaks along `angle` for `length` pixels, or,
with `follow`, along the way the layer moved: its speed in canvas pixels per second times
`shutter` seconds, at most `length` pixels; the scene draws one more frame once the layer stops so
that the streak goes away.

Blurs and distortions treat the edges of the screen like the edges of the picture: a layer that
reaches past the screen, or fills it, keeps the color of its outermost visible pixels instead of
fading into the empty space around it, on all four sides.

`tint` mixes `color` into the layer with a `blend` mode (multiply by default) by `amount` (0 to 1,
times the alpha of the color), or with `mode: 1` colorizes it: the color takes the brightness of each
point and `blend` does not apply. `blend_texture` blends `texture` (a picture or a layer) over the
layer with `blend` by `amount` times the alpha of the texture, placed on the layer, moved by `offset`
and repeated by `scale` (fractions of the layer); a layer may chain several, each one a pass, and the
effect never makes the layer more opaque. `light_shafts` draws beams of its own, made of noise
across their direction: `direction` 0 sends them along `angle` (degrees, 90 downwards), 1 from
`center` and 2 from the corner of the layer nearest to `center`; `exponent` and `smoothness` (0 hard
edges, 1 soft) shape them, `feather` fades their far end, `intensity` multiplies them and `color` or a
`gradient` (read from the start of the beams to their end) colors them. They add light, so they also
show on a transparent `solid` layer, and the scene only draws new frames for them while `speed` is
not zero.

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
| `pace` | when a time effect changes: `{ "rate": "<float parameter>" }` (that many times a second), `{ "sweep": ["<speed>", "<gap>", "<phase>"] }` (a band crosses the layer once every `1 + gap` turns and waits off it in between), `{ "steps": [<high>, <low>] }` (1 to 240 times a second, the second number at Q0) or `"outside"` (only while the pointer is off the output); the scene then sleeps until the picture changes |
| `mask_mode` | `mix` (default) blends the result with the input by the mask; with `strength` the shader scales itself by `mask_at(uv)`, as distortions should |
| `params` | at most 16 of `float`, `vec2`, `color`, `bool`, `texture` and `lut`, with `id`, `default`, `min`, `max` and `uses` (`time`, `cursor`, `clicks` or `audio`; clicks come with the pointer), an input needed only while the parameter is not zero (a `follow` flag needs the pointer only when set, a `vec2` while either part is not zero); parameter *n* is `param(nu)`, in `.x`, `.xy` or `.rgba` (linear) |
| `passes` | `{ "entry", "scale", "inputs", "output" }`; `scale` is a fraction of the layer size, and lower quality levels shrink every pass but the last; `inputs` (default `["previous"]`) name `previous`, `input`, `source` (the layer before its effects), `mask`, `pass:N`, `state`, `backdrop` (what is already drawn under the layer, which then draws on its own) or `asset:<texture parameter>`, bound to `input_tex` and then `aux0` to `aux3` |
| `cost` | `{ "taps", "passes", "scale" }`; taps × passes × scale² units, light up to 1, medium up to 6, heavy above |
| `min_fps` | 1 to 240: the frame rate at which the effect still looks smooth; while the only things that move in a scene are effects that declare it, the scene runs at the highest of their `min_fps` instead of the full rate (never above the usual limits) |
| `velocity` | a `bool` parameter that, when set, gives the shader the speed of the layer in `fx.velocity.xy` (canvas pixels per second) and keeps the scene drawing while it moves |
| `fuse` | `uv` or `color` for single-pass effects, see below |
| `reach` | a float parameter, in pixels, by which the effect grows the layer bounds |
| `state` | `{ "scale", "settle", "fade" }`, persistent textures for simulations, see below |
| `solve` | `homography`: the first four parameters are `vec2` corners (top left, top right, bottom right, bottom left, as fractions of the layer) and the three slots after the parameters hold the rows of the matrix that takes a layer point back to the unit square; such an effect is not fused |
| `quality` | overrides per level, `{ "q1": { "params": { "taps": 8 }, "scale": 0.5 } }`, applied at that level and below; a number is an upper limit for the value the scene sets, other values replace it, `scale` shrinks the inner passes and the state, and `"freeze": true` keeps an effect that would be dropped at that level: its state stops and it shows its last picture without asking for frames |

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
`hash2`, `noise1`, `value_noise`, `gradient_noise`, `fbm(p, octaves)`, `bayer(pixel)`,
`layer_extent()` (the size of the layer in output pixels), `time_phase(t, uv)` (the brightness of a
`phase` picture at that point), `outside_layer(uv)` (the point, or `OUTSIDE_UV` when it is off the
layer; a `uv_<id>` function that returns `OUTSIDE_UV` leaves that point transparent) and
`relief(depth, sampler, uv, shift, focus, steps)`, the depth search of `depth_parallax`.
`fx.texel` is the size of the first input, `fx.info.z` the pass index, `fx.area` the effect
rectangle in output pixels, `fx.clip` the part of it that is on the screen (as fractions, where
`sample` and `tap` stop reading) and `fx.velocity` the speed of the layer when the descriptor
names a `velocity` parameter. Effects may not declare bindings of their own.

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
scene goes still, and also after the layer or the effects before it last changed. `fade` names a
float parameter by which the state is kept each step: the scene then also renders until a picture
kept that way has faded below 1/512 (at most 60 s). In a built-in effect, passes with `"output":
"step"` before the pass that writes the state run with it on each step, into 16-bit textures of
their own, so that a simulation can solve in several passes; package effects use targets instead.

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
| `params` | at most 24; besides the kinds above, `int` (whole numbers), `enum` (with `values`: 1 to 64 `{ "value", "label" }`), `blend` (one of the 34 blend modes, given to the shader as its number in the order of the list), `position` (a point of the layer the editor lets the author place) and `mask` (a picture painted per channel, with `channels` 1, 3 or 4 and `paint`, the color or number a mask without a picture has); a `texture` or `mask` whose value names no asset but is `white`, `black`, `noise` or `normal_flat` reads that generic picture; every parameter may have a `label` and a `help` per language, of at most 256 characters, and `yields_to`, another parameter that, while not zero, takes over: this one then asks for no input of its `uses` |
| `targets` | at most 8 named textures `{ "scale" (0 to 2 of the layer size), "format" }` with `rgba8` (default) or `rgba16f`, kept between passes of one frame |
| `options` | at most 8 variants `{ "id", "values", "default", "when_bound" }`, each a WGSL `override` of the shader with 1 to 16 whole numbers or flags; a use sets them through `params`, as literals or bound to a property, and every combination it may need (at most 8 per use) is built when the scene loads, never during a frame; `when_bound` turns an option on while the named texture parameter has a picture |
| `compose` | `true` draws the layer over what lies below it first; the effect reads that picture as its input and its result replaces the layer's box on the scene |
| pass `target` | writes the pass into that target instead of the next texture; targets start transparent each frame, so passes that write them with a `blend` add up |
| pass `copy` | a pass without a shader that copies one input (`"copy": "target:history"`) into its output, resizing it when the sizes differ |
| pass `swap` | a pass without a shader that exchanges two targets of the same scale and format (`"swap": ["front", "back"]`), for ping-pong passes |
| pass `blend` | `replace`, `normal`, `add` or `multiply` onto its output, to accumulate without a copy |
| pass `vertex`, `grid` | a vertex entry point and a grid of 1 to 64 cells a side over the layer, to bend it (`vertex_effects`) |
| pass `inputs` | at most 9, also `backdrop` (what is already drawn under the layer, which then draws on its own), `target:<name>` and `asset:<mask parameter>`; they are bound to `input_tex` and then `aux0` to `aux7` |

A package effect has at most 16 passes. Its shader is checked for the work it can do before anything
reaches the graphics driver, on a thread of its own with a 64 MiB stack, so that no shader can
exhaust the stack of the program that reads it. Before naga parses it, a scan of its text refuses
an expression more than 256 operations deep (`a + a + ...`, unary signs, member accesses, indices
that follow a bracket such as `a[0][0]`, `else if` chains and the brackets around them all count; a
number ends before a `.` that a letter follows), more than 64 nested brackets of any kind and more
than 4 096 arguments to calls and constructors in the whole shader; a `//` comment ends at every
line break WGSL knows (`\n`, `\r`, vertical tab, form feed, U+0085, U+2028, U+2029) and the
Unicode blank space WGSL allows counts as blank. The same limits hold again on the parsed shader,
whatever its text: an expression more than 256 operations deep (a named `let` starts a new one),
more than 64 nested blocks and more than 4 096 arguments and constructor parts are refused.
Then: at most 32 768 expressions; every loop counts an `i32` or `u32`
variable from a literal start to a limit that is a literal, a constant, an `override` with listed
`options`, or `min(x, <literal>)`, steps it once in the loop's `continuing` part by a literal or a
constant of at least 1 and never writes it elsewhere inside the loop (`for (var i = 0u; i < 16u;
i++)` qualifies; float counters, steps that come from an `override` or `min`, `while` loops, `break
if` and counters passed to functions or compared through a conversion do not); a conversion
such as `u32(m)` in a start or a limit counts only when the value it converts is known and fits the
new type unchanged, and `bitcast` never counts; the start, the limit and the value the counter has
when the loop ends must fit its type, so `i >= 0u` counting down, `i <= 4294967295u` or
`u32(-1i)` are refused; a loop runs at most 1 024 times. Every entry point is weighed in scalar
operations a pixel, nested loops and called functions included: one per scalar of a value an
expression computes, loads or stores and one per statement, a matrix product as rows × columns ×
the shared size, `sin`, `exp`, `sqrt`, `normalize` and the like as four, an integer division or
remainder as 32 and a float remainder as four per component, a texture read as 16 and
a call as all of the function it calls; the statements of a loop before its condition count once
more than its rounds, since they run once more. A read or a write at a computed index into a function or
`private` array or matrix that the shader writes element by element or passes by pointer costs
F × min(F, 1 024) / 32 more, where F is the size in bytes of all the arrays the shader indexes that
way together (32 768 for 1 KiB): such arrays leave the registers, and a scattered access to them
costs as much as thousands of operations; a read at a computed index into a `const`, into a
variable only ever written whole, or into an array value costs as much as a texture read;
a function never receives a pointer into a function variable or to an element picked that way. An
entry point, and the `material` or `particle` function of a stage effect, may run at most 1 048 576
operations a pixel, and may be at most 32 768 operations long once every call counts the whole
function it calls again, since drivers copy called functions into their callers; the passes of an
effect together, vertex entries included, are at most 131 072 operations long. Function and
`private` variables, every array and structure value the shader computes (call results, loads,
constructors and arguments included, all counted at once) and the deepest chain of calls may keep
at most 8 KiB a pixel. The
check is an early, readable refusal, not the only safeguard: the engine also builds every package
shader with a hidden counter in each of its loops, so any loop stops after 1 024 rounds each time
it starts, and every loop a pixel runs stops once the loops of that pixel have made 65 536 rounds
together, whatever the check concluded. Option values must fit the type of their `override`.
Targets are at most twice the layer size and never larger than the largest texture of the device,
and parameters fill 24 slots (`param(0u)` to `param(23u)`). The descriptor's `cost` counts at least
one unit for each pass that draws, times the square of its scale, nine times for a pass with a
`grid` (twice for a grid of one cell), whose operations and texture reads count nine times too, one unit for every four texture reads its shader can make a pixel and one unit for
every 512 operations, loops and the stage function of a `material` or `particle` effect included,
whatever the descriptor says; its numbers must be finite and
at least 0. A layer whose effects read `backdrop` adds two units for the copy, and four for a
`compose` effect. The engine shrinks every triangle a vertex entry draws, around its middle, to at
most three cells of the grid a side, so a grid that stretches its cells over the layer draws each
pixel nine times on average at most; a vertex entry that bends the grid moves its cells freely.

On the desktop, the daemon and its workers, the app with its editor, `wprs-ctl` and `wprs-server`
run that check (the scan of the text, naga's parse and validation, the limits above and the rewrite
that adds the loop counters) in a short-lived helper first: a copy of their own program
(`/proc/self/exe`, so it still works after an upgrade replaced the file) started with the hidden
argument `--wprs-check-shader` and an empty environment, which limits itself to 1 GiB of address
space, 4 seconds of processor time and no core file (or the lower hard limits it inherited; when it
cannot lower them it refuses with `the checker could not limit itself: <error>`) before it reads the id, the descriptor and the
shader on its standard input, and answers with the hash of what it checked and its refusal, if any.
It is killed after 10 seconds. The daemon's workers, which lower their own priority, start the
helper from a thread that keeps the priority they started with. Only when the helper accepted exactly those files does the program
check them again itself (which also gives the descriptor its measures), and only effects that passed
that check are ever built for the graphics card. A refusal of the helper refuses the package with
the same reason; a helper that crashes, runs out of memory or processor time, takes too long or
does not start refuses it with `its shader could not be checked: the checker ...`, without a
strike, since the program itself never ran the check. Verdicts are kept by a SHA-256 of the program
version, the limits, the id, the descriptor and the shader, in memory and in `checked-effects`
beside `stopped-effects` (lines `<hash> ok` or `<hash> refused <reason as a JSON string>`, the
4 096 used last, rewritten under a lock and renamed into place, with up to 4 096 kept in memory), so the helper runs once for each version
of an effect, when it is first read, and a new release checks every effect again; failures of the
helper are kept nowhere, and the daemon reads the library packages they refused again when it starts
and on `RetryEffects`. The phone keeps the check in its own process, bounded by the limits of naga's
constant evaluation.

The engine builds every pass and every variant a use may need when the scene loads, each inside an
error scope of the graphics driver, on up to four threads beside the one that draws, and waits for
them at most 10 seconds. The package shaders a scene builds may be at most 262 144 operations long
together, each pass counted once for each variant built; the effects past that are left out. An
effect the driver refuses, or whose shader is not built in time, is left out and the layer is drawn
without it; so is every package effect drawn in a frame the driver rejects. Only the variants built
then are drawn: an option value a property binding does not map draws the built variant of the
lowest mapped value, and a use whose variant is missing is left out with `its shader was not built
for these option values` instead of building on the thread that draws. A program that ends cleanly
while a package shader still builds (the daemon on `SIGTERM`, its workers, the app, `wprs-ctl`)
stops the other builds, waits at most a second for the driver and then exits at once, without the
exit handlers that would race the driver.

Each view keeps at most two frames on the GPU: before it sends a frame it waits for the one two frames
back. A frame with package effects, or of a view with effects on trial, may take at most 10 seconds,
and so may every other wait for the GPU while such a frame was sent less than 20 seconds before and
has not been seen to finish, such as composing it into a stream; other frames and waits may take 10 seconds on a GPU and, with software
rendering, 10 seconds times 32 divided by the number of cores, between 10 and 60 seconds (20 with 16
cores, 40 with 8, 60 with 4 or fewer; `LP_NUM_THREADS` counts when set). A frame with package effects still running after its
limit leaves out the package effects of the
frames of that view still on the GPU and of its trials, with `it kept the graphics card busy for more
than 10 s`, counts a strike for each and fails with the GPU stopped. With software rendering a scene
draws at most a megapixel per core (an 8K screen with two cores draws a 1080p frame, scaled up), and
when a measured frame takes more than an eighth of its limit the view draws the next ones at 70 % of
the size, down to a quarter, growing back the same way once frames take less than a sixteenth of
that, so a legitimate heavy frame never reaches the limit. The engine also measures the GPU time of
every frame and of each effect: where the device can write timestamps it times the whole frame, the
particle simulation and sorting sent before it, and each run of passes of one effect, built in or
packaged, on every frame (31 runs a frame; a frame
with more takes turns among them, and then shares the time of the frame not spent in timed built-in
effects among the timed package effects by their part of the timed time), and reads the times back
when the frame has finished, without waiting for it; the GPU time of a frame is the frame plus its
particles, and `wprs-ctl scene bench` shows it as `gpu` and the particle part in its simulation line.
Elsewhere it waits for the GPU before and after every fifteenth frame of a scene that draws (and
around its particle simulation), every frame with package effects of a trial or while one of them is
suspected, and counts the whole frame. When the package effects of a frame take more than 400, 60, 40
or 30 ms on the four tiers on their own (a fifth more than a frame at 30 or 40 frames a second, so
effects right at the limit are not left out by noise), the package effects that take the most of it,
until the rest of their time fits, are left out with `it is
too slow for this graphics card: a frame took <total> ms on it, <n> ms of them in this effect, and it
allows <limit> ms` once that happened on three frames within 8 seconds, cheap frames in between or
not, or at once for an effect over four times the limit (and 100 ms), whatever the rest of the frame
costs. Built-in effects are timed but never left out, and what layers and built-in effects cost never
counts against a package effect: the quality level is what makes such a frame fit. Without timestamps only a frame over a second (and four
times the limit) leaves its package effects out. A slow effect is left out for as long as the program runs, also in its other views,
and counts no strike: it may run well at another size or on another day. Only a frame that keeps
the GPU busy for 10 s, a lost GPU or a crash counts strikes. The desktop reports it
in `GetState().outputs[x].left_out` while the scene keeps playing, so the app, the tray and
`wprs-ctl state` show `the effect <id> was left out: <reason>` as a note, and the phone lists it
among its diagnostics.

Package effects that stop the graphics card or the program count strikes, by a hash of their
descriptor and shader, in `stopped-effects` in the cache folder of the desktop (the daemon, its
workers, the app and `wprs-ctl` share it) or in the files of the phone. The newest 1024 effects
with one strike are kept, and the newest 4096 blocked ones; single strikes never push out a blocked
effect. Before a program checks a package shader for the first time (after the helper accepted it, so only its own check counts) it writes
`<hash> trying <process>` there and removes the line once the check ends; before it builds the effect for a scene
it writes the line again and removes it once the first three frames that follow have finished on the
GPU, once a second has passed since the first one finished with no frame on the GPU (a still scene
draws once), or once the scene stops drawing. Each process that writes such a line holds a lock on a file named
after it in `stopped-effects.trials` beside the book for as long as it runs, so processes in other
pid namespaces (separate sandboxes sharing the cache folder) see each other's trials alive. A line
whose process no longer holds its lock counts as a strike the next time any of them reads the file,
so a crash, an abort, an out-of-memory kill or a hang the user ends while an effect is checked,
built or first drawn counts like a lost GPU. A clean exit (the daemon stopped or its display server
gone, a worker, the app or `wprs-ctl` ending) removes the lines of the process first. A running
process reads the file again whenever it changes, so a block set by another process holds at once.
When the GPU stops (the device is lost, or runs out of memory, or a resource it made turns out
invalid) while frames are in flight, the effects of the frames that had not finished count a
strike, not those of outputs whose frames had (each frame is followed until the GPU reports it
done); a shader still building after 10 seconds counts one too. An effect with
two strikes is no longer checked (`its shader is not checked again`, so the package is refused) or
built (`the graphics card or the program stopped twice while it built or drew this effect`); an
effect that reaches two strikes while it is drawn is left out with `the graphics card or the
program stopped while it drew this effect`. A single strike is forgotten after a day, since a
suspend or a driver reset also stops the GPU. `wprs-ctl retry-effects`, `RetryEffects` on D-Bus, Try
again next to the note in the app or Try the left-out effects again among the diagnostics of the
phone forget them and draw the scenes again, building every effect that was left out (also those
whose shader took too long to build, the driver refused or was too slow, which are measured
again); library packages refused
because a shader was not checked again are read again (also each time the daemon starts), and a
refusal from the book is never kept in the cache of checked shaders. A new version of the effect has
another hash and runs again.

Layers with a `compose` effect or a pass that reads `backdrop`, built-in or not, and layers whose
effects take a layer as texture or mask are drawn again whenever anything in the scene changes, since
what lies below them or the layer they read is part of their picture. A layer that feeds others is
drawn once a frame however many effects read it, and a change to it, hidden or not, or to a layer it
reads in turn, redraws the layers that read it.

The contract is the prelude above plus `aux4` to `aux7` (eight auxiliary textures in all), `mask_rgb(i, uv)` (every
channel of `aux<i>` placed on the layer, for `mask` parameters, which the scene editor paints one channel at a time), `PI`,
`tex_size(i)` (size in pixels of `aux<i>`), `daytime()` (0 at midnight to 1, in steps of a
minute), `parallax()` (the current parallax shift, before each layer's own depth), `cursor_last()`
(the pointer one frame earlier, in fractions of the output), `grid_uv(i)` (the corner of the grid
that vertex `i` of a pass with `grid` draws, six vertices a cell), `grid_position(uv)` (that point
of the layer as a clip-space position), `layer_color()` (the tint of the layer times its
brightness, linear) and `layer_alpha()` (its opacity),
`band_left(i)` and `band_right(i)` (64 bands per channel), `band_of(i, n)`, `band_left_of(i, n)`
and `band_right_of(i, n)` (band `i` of 16, 32 or 64, mixed or per channel), `rotate2(v, degrees)`, `greyscale(rgb)`, `hsv(rgb)`, `from_hsv(hsv)` and `blend_mode(mode, b,
s, amount)`, which blends straight colors with any of the blend modes. A vertex entry point returns
`VertexOut(position, uv)`, usually `VertexOut(grid_position(bent), grid_uv(i))`. `CONTRACT` in
`crates/wprs-scene` holds it, and package shaders are checked against it whole. Package effects run
the same WGSL on every platform.

The scene editor imports an effect with its two files (`wprs-ctl scene import-effect <dir>
<descriptor.json> [--wgsl <shader.wgsl>]`, or Import an effect in the app), copies them to
`effects/<id>.json` and `effects/<id>.wgsl`, refuses them with the reason when they fail the checks
above, and then lists the effect among the ones a layer can add. Its parameters become controls in
the inspector with their localized `label`, sliders within `min` and `max`, choices for `enum` and
`blend`, and may be bound to properties, keyframes or scripts like any parameter.

### Newer effects and parameters

The effects and parameters below came after the first built-in set. Each names a feature:
`effect.<id>` for an effect, `effect.<id>.<parameter>` for a newer parameter of an older effect, so
that older engines refuse a package that uses one with the newer-version message instead of an
unknown name; every engine that knows them draws them on every platform. Package effects may not
take their ids. Positions are fractions of the layer, `blend` parameters take any of the 34 blend
modes, `phase` parameters are `linear` pictures whose brightness shifts the time of each point (by
a whole cycle at full brightness, by eight steps of the noise for `shake`) so that neighbouring
parts do not move together, and `gradient` parameters are pictures of one row read from left to
right. An effect that has a `phase` picture is not fused with its neighbours. Costs are the units
of "Cost and frame rate": light effects run at every quality level, medium ones are dropped at Q0
(and at Q1 when `optional`), heavy ones run their inner passes at a quarter of their size at Q1;
the quality level falls on battery, with software rendering or when frames take too long, so these
rules are also what a phone or a laptop on battery draws.

| Effect | What it does | Parameters | Cost and lower levels |
|--------|--------------|------------|-----------------------|
| `blend_gradient` | mixes in another picture through a gradient map, from left to right without one | `texture`, `map` (textures), `progress`, `softness` (0 to 1) | light |
| `caustics` | moving light patterns of water | `color`, `scale` (0.1 to 16), `speed` (0 to 8, needs time while not zero), `strength` (0 to 4), `perspective` (0 to 1) | light; one layer of pattern instead of two at Q0 |
| `displace` | bends the layer by a normal map (`mode` 0, refracted through its slope) or an offset map (`mode` 1, 0.5 still), repeated over the layer, or by built-in noise without one | `map`, `mode`, `scale` (0.1 to 16), `strength` (0 to 256 px), `scroll` (−4 to 4 a second, needs time while not zero), `ratio` (0.1 to 10, wider than tall) | light |
| `electric` | thin crackling lines where two drifting noises meet | `color_a`, `color_b`, `bounds` (0 to 1, where lines start and are full), `scale` (0.1 to 16), `smoothness` (0 to 1), `speed` (0 to 8, needs time while not zero), `multiply` (0 to 8), `blend` (`add`) | light; changes 30 times a second and asks for a frame only then, 15 times at Q0 |
| `fire` | lifts and flickers flames already in the picture, and colors them by brightness with a `gradient` | `speed` (0 to 8, needs time while not zero), `strength` (0 to 64 px), `scale` (0.1 to 16), `gradient` | light |
| `fluid` | smoke, fire or liquid simulated over the layer, from one point emitter, one line emitter and the pointer | `render` (0 gradient, 1 emitter color, 2 the picture itself, 3 distortion only), `gradient`, `color`, `velocity_decay`, `dye_decay` (0 to 1, kept each 1/60 s), `push` (0 to 8, needs the pointer while not zero), `source`, `source_angle` (−360 to 360, −90 is up), `source_strength` (0 to 64), `line_from`, `line_to`, `line_strength` (0 to 64; a strength above zero needs time), `blend`, `perspective` (0 to 1), `opaque` | heavy: a state of a quarter of the layer, 20 pressure steps every 1/60 s (8 at Q1, where the state is halved); frozen at Q0, where it shows its last state and asks for no frames; draws only while it emits, the pointer pushes it, or for 6 s after |
| `glitter` | short star-shaped sparkles in a grid of cells | `density` (0 to 1), `size` (0.5 to 16 px), `speed` (0 to 8 new patterns a second, the scene sleeps in between), `color` | light; cells twice as large at Q0 |
| `lens_flare` | a core, a halo and ghosts along the line from a light to the middle | `source`, `follow` (needs the pointer when set), `intensity` (0 to 4), `ghosts` (0 to 8), `halo` (0 to 1), `color` | medium |
| `local_contrast` | deepens contrast around edges (unsharp mask) | `radius` (1 to 64 px), `amount` (0 to 4), `luma_only` (on by default: colors keep their hue) | medium, two blur passes at half size; best marked `optional` |
| `pulse` | makes brightness, color or opacity beat in time, with noise, or with a band of sound | `what` (0 brightness, 1 color, 2 opacity), `speed` (0 to 16 beats a second, needs time while not zero and no band drives the beat; a band replaces it), `bounds` (0 to 4, the level at the low and the high of a beat), `tint_low`, `tint_high`, `noise` (0 to 1, needs time while not zero), `noise_speed` (0 to 16), `phase`, `audio_band` (0 off, or a band 1 to 64 whose level drives the beat; needs sound only then) | light |
| `reveal` | shows another picture through a circle or an ellipse that may follow the pointer | `texture`, `shape` (0 circle, 1 ellipse), `radius`, `softness` (0 to 1), `follow` (needs the pointer when set), `invert`, `center` | light |
| `spin` | turns the part of the layer around a center, fading out towards `radius` | `center`, `angle` (−360 to 360), `speed` (−720 to 720 °/s, needs time while not zero), `radius` (0 to 2, of the longer side) | light |
| `trail` | keeps fading copies of what the effects before it drew, the same at any frame rate | `persistence` (0 to 0.99, kept each 1/60 s) | medium, a state of the layer's size; draws while the layer or the effects before it change and until the copies have faded |
| `transform` | moves, scales, turns or skews the layer after the effects before it, within its box | `offset` (−2 to 2), `scale` (0.01 to 16), `angle` (−360 to 360), `skew` (−89 to 89), `pivot` | light |
| `vhs` | a rolling tracking line, torn rows, snow and color fringes of a worn tape | `strength` (0 to 1), `speed` (0 to 8, needs time while not zero), `noise` (0 to 1), `aberration` (0 to 16 px) | light |

| Effect | Newer parameters |
|--------|------------------|
| `bloom` | `tint` (white) colors the glow, as `canvas.bloom` does |
| `blur` | `blend` (`normal`) and `amount` (0 to 1) mix the blurred picture back into the sharp one |
| `blur_motion` | `follow`, `shutter` (0 to 0.5 s) |
| `chromatic_aberration` | `mode` (0 grows with the distance from `center`, 1 the same shift along `angle`, 2 the same shift away from `center`, 3 grows with the square of the distance), `angle` (−360 to 360) |
| `film_grain` | `mono` (black and white film), `bias` (−1 to 1, more grain in dark or bright parts), `blend` (`overlay`, the lightness grain as before; any other mode blends a grey noise) |
| `flow` | `noise` (a built-in flow field when no map is given, as long as `direction`), `phase` |
| `god_rays` | `mode` (0 radial, 1 along `angle`, 2 a star of `edges` rays from every bright part, at most 8 at Q1), `angle` (−360 to 360), `noise` (0 to 1, a flicker that needs time while not zero), `edges` (2 to 16) |
| `iris`, `lens_distort` | `background`: where the layer moved away it shows what lies behind instead of its edge |
| `reflection` | `angle` (−360 to 360) turns the mirror line around its middle |
| `scroll` | `repeat_x`, `repeat_y`: off, the picture leaves that side instead of coming back |
| `shake` | `noise` (0 to 1, jerkier), `friction` (0 to 1, longer near rest), `bounds` (0 to 1 of `strength` on each axis), `phase` |
| `shine` | `blend` (`add`, as before), `gradient` (read across the band) |
| `sway` | `double` (a second, slower wave over the first), `phase` |
| `twirl` | `aspect` (0.1 to 10, an ellipse), `inner` (0 to 1, turned whole inside), `noise` (0 to 1), `repeat` (wraps the picture around instead of stretching its edge) |
| `water_ripple` | `perspective` (0 to 1, smaller waves towards the top), `reflection` (a picture seen in the slopes), `tint` (colors the moving water), `walls` (a picture where the water stays still), `speed` (0 to 4) |
| `waves` | `count` (1 or 2), `angle2` (−360 to 360, of the second wave), `perspective` (0 to 1), `phase` |

The values left at their defaults draw exactly what the effects drew before them.

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
plays from where its timelines are, and a sound started again starts over. A layer keyed on a
timeline that is paused or has ended keeps playing on, and jumps back to the timeline when it plays
again; a timeline started late or sought starts its sounds at its position, not from zero. The
engine re-aligns playing sounds with the scene once a second. About 50 ms pass between the scene and
the speaker (the soundtrack asks PipeWire for 20 ms periods and keeps about 20 ms queued); positions
are counted from the playing position of the audio output, plus one 20 ms PipeWire period when the
sound goes to PipeWire, so the queue does not shift sounds.

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
warm-up frames. When they run on the processor, the state of the output says `"particles": "cpu"`
(`"gpu"` otherwise) and the app, the tray, `wprs-ctl` after showing the scene and the settings of
the phone say so, because fewer of them are shown.

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
| `init.gradient` | at most 8 `[t, color]` stops in order, read at a random place (leaned by the exponent of `color`) and replacing `color`; not with `hsv` or `palette` | `particles2` |
| `init.velocity`, `radial`, `inherit` | `[[vx0, vy0], [vx1, vy1], exponent?]` replacing `speed` and `angle`; a speed away from the center of the shape; `{ "from" (cursor or control point), "speed": [min, max] }` | `particles2` |
| `init.turbulent`, `offset_noise`, `remap` | turbulent starting velocity `{ "forward", "scale", "noise_speed", "offset", "speed", "phase", "audio" }`; a fractal offset of the birth `{ "distance", "scale", "sign", "octaves" (1 to 6), "noise_scale", "noise_speed" }`; at most 8 `{ "input", "in", "clamp_in", "output", "out", "clamp_out", "op", "wave" }` that map `daytime`, `runtime`, `life`, `age`, `speed`, `random`, `distance:pN`, `audio:<band>` or `cursor:x\|y` onto `size`, `alpha`, `color`, `speed`, `life`, `rotation`, `spin` or `frame` with `set`, `mul`, `add` or `sub` and an optional `sine` or `noise` wave | `particle_fields` |
| `perspective`, `init.depth`, `init.depth_speed` | `{ "focal" (1 to 100000 px) }` and depth ranges in pixels, which need `perspective` | `particle_depth` |
| force `window` | `[in_start, in_end, out_start, out_end]` fractions of the life, in order, in which any force fades in and out | `particles2` |
| forces | `{ "oscillate": { "what": "position" \| "alpha" \| "size", "freq", "scale", "phase", "mask" } }`, `{ "cap": speed }`, `{ "torque": k }`, `{ "spin_drag": k }`, `{ "slow_near": { "target", "inner", "outer", "factor" } }`; `attractor`, `vortex` and `orbit` take `radius` and `falloff` (`constant`, `linear`, `inverse_square`), `vortex` also `inner`, `outer` and `ring: { "radius", "width", "pull" }`, `turbulence` also `speed`, `time_scale` and `mask` | `particles2` |
| forces | `{ "keep_distance": { "target", "distance", "strength" } }`, `{ "keep_between": { "from", "to", "strength" } }`, `{ "remap": { … } }` applied every step (input `age` too) | `particle_fields` |
| force `boids` | `{ "radius", "separation", "alignment", "cohesion", "max_speed" }` flocking, for systems of at most 8192 particles | `boids` |
| collider `mask` | `{ "mask": { "texture", "threshold" } }`, the part of an image stretched over the canvas whose alpha is above `threshold` (0.5 by default), which particles bounce off or die on | `particles2` |
| colliders | `{ "box": { "from", "to", "inside" } }`, `{ "segment": { "from", "to" } }`, `circle` with `inside`, and `stick: true` | `particle_collisions` |
| collider `layer` | `{ "layer": { "id" } }`, the opaque shape of a layer | `particle_layers` |
| `children` | at most 4 `{ "system", "on": "static" \| "spawn" \| "death" \| "follow", "probability", "max", "offset", "angle", "scale" (0 to 100), "points_from", "inherit", "follow_inherit" }`: other systems born with or from each particle, at most two levels deep and never back to themselves; `inherit` copies `color`, `size`, `alpha` or `velocity`, every step with `follow_inherit` on `follow` children; a collider that `kill`s counts as a death | `particle_children` |
| render `brightness`, `cycles`, `stretch` | 0 to 16, may be bound; repeats the frames that many times per life (1 to 64, with `frames: life`); `stretch` may be `{ "time", "min", "max" }` in pixels, and `min` equal to `max` only turns a sprite along its motion | `particles2` |
| render `order` | `newest`, `oldest`, `largest`, `smallest` or `lowest`: the particles drawn on top; others are drawn in no set order | `particles2` |
| render `rope`, `trail` | `{ "subdivide" (0 to 7), "width", "uv_scale", "uv_scroll", "smooth" }` joins particles in birth order; trails of up to 64 steps with `segments`, `subdivide`, `uv_scale`, `uv_scroll` and `textured` | `particle_ropes` |
| render `cutout`, `orient`, `angle` | `[start, end, opacity]` remaps the alpha of the texture; `screen`, `velocity` or `fixed` with `angle` | `particle_looks` |
| render `lit`, `normal`, `refract` | lit by the scene lights with a normal map; `{ "normal", "amount" }` bends what is behind | `particle_lighting` |
| render `shader` | a package effect with `stage: "particle"` | `particle_shaders` |

The newer fields behave as follows.

- **World space.** A `world` system moves in canvas pixels: the transform of its layer places,
  turns and scales particles, their speed and their shapes when they are born, and they are drawn
  without it, so moving the layer afterwards leaves them where they are. Gravity and the other
  forces act along the directions of the canvas, and targets, colliders and control points of the
  layer move with it. Each layer that draws a `world` system moves particles of its own.
- **Control points.** `at` is in pixels of the layer, or of the canvas with `space: world`; a point
  that follows the cursor goes back to the center of the canvas when the pointer leaves. A shape
  with `at` is drawn around that point, turned by its `angle`. The `points` of a layer's `instance`
  and the `point(n, [x, y])` of scripts move points `p0` to `p7` for that layer.
- **Shapes.** `inner` cuts a box of that size out of the middle of a `box`, which births fill evenly.
  `scale` stretches a `circle` into an ellipse and `sign` keeps its positive (1) or negative (−1)
  half along x and along y.
- **Starting values.** A range `[min, max, exponent]` picks `min + (max − min) · u^exponent`.
  `alpha` replaces the alpha of the color. `hsv` picks the hue in degrees (with `steps`, one of that
  many hues from one end of the range to the other), the saturation and the value from 0 to 1;
  `palette` picks one of its colors and moves its hue by up to `noise[0]` half turns and its
  saturation and value by up to `noise[1]` and `noise[2]`. `radial` adds a speed away from the center
  of the shape (the center of the canvas for `edge` and `mask`, the direction of `angle` for a
  `point`), and `inherit` adds the velocity of the pointer or of a control point, times a number
  picked from `speed`.
- **Forces.** `window` fades a force in between its first two fractions of the life and out between
  the last two. `attractor`, `vortex` and `orbit` weaken with distance as the inverse square, the
  inverse and not at all by default; with a `radius` they do nothing beyond it and weaken as
  `1 − d / radius` (`linear`) or its square (`inverse_square`). A `vortex` with `inner` and `outer`
  turns fully inside `inner` and fades out at `outer`, and its `ring` pulls particles toward a circle
  of that `radius` with `pull`, fully at `width` pixels away. `turbulence` scrolls its noise `speed`
  pixels per second to the right, changes it `time_scale` times as fast and scales its x and y by
  `mask`. `oscillate` picks a frequency, a scale and a phase for each particle and moves it around
  where it would be without drifting (x and y a quarter of a turn apart, scaled by `mask`), fades its
  alpha by up to the scale or grows and shrinks its size by that fraction. `cap` limits the speed,
  `torque` adds degrees per second to the spin each second, `spin_drag` slows the spin and
  `slow_near` multiplies the speed every 1/60 s by `factor` near its target, fading to no effect
  between `inner` and `outer`.
- **Emitters.** `delay` and `duration` count from the start of the system or from the last
  `play()`; a finished emitter, with no particle alive, lets the scene sleep. `periodic` alternates
  emission and pause for times picked from the seed. One-time bursts fire whenever the emitter
  starts: at first, after its `delay`, at each `periodic` window and after `play()`. `max_per_step`
  keeps the rest of the births for the next steps.
- **Children.** `static` children emit on their own around the origin of their parent. `spawn` and
  `death` children start the one-time bursts of their emitters (one particle when they have none, at
  most 1000) where each particle is born or dies, in the same step, and `follow` children are born
  `rate` times a second at each living particle, ride along with it unless their system is `world`,
  and die with it. Shapes of children are placed around the particle, moved by `offset` and turned
  by `angle`; `scale` also scales their sizes. `probability` is drawn once per particle and `max`
  limits the children born in a step; at most 1024 births and 1024 deaths of a system start children
  in a step. `points_from` takes the control points of the parent.
- **Instances.** `alpha`, `size`, `brightness` and `color` multiply the look of the layer; `count` is
  the fraction of births and slots it keeps, `speed` and `life` multiply the starting speed and life
  and `time_scale` the speed of its time. A layer whose `instance` sets `count`, `speed`, `life`,
  `time_scale` or `points` moves particles of its own; the others share them. A system may `lock`
  what layers and scripts may not change.
- **Looks.** `render.brightness` multiplies the color with the brightness of the layer, `cycles`
  repeats the frames over the life and the `stretch` range limits how long a particle is drawn along
  its motion, in pixels.
- **Frames.** A texture whose asset has `rects` draws each frame from its own rectangle, so frames
  may differ in size; an animated picture is split into its own frames the same way.
- **Gradients.** `init.gradient` picks a color between the two stops around a random place and the
  first or last color outside them.
- **Trails.** `render.trail` keeps the last `length` positions of each particle, one per 1/60 s
  step, and draws a ribbon through them that is `width` times as wide as the particle was there and
  fades to nothing at its end, with the color and size the particle had there. It holds 16 bytes
  per particle and step, at most 16 steps (4, 8 or 16 by device class, none with software rendering
  or on the CPU of the weakest devices, half of them or none at lower quality levels) and at most
  1 048 576 points per system. A layer with a `render` of its own draws as many of the kept
positions as its own `trail` asks for, or none.
- **Order.** `render.order` draws the particles from the bottom up in 256 steps of age, size or
  height, which matters with the `normal` blend; it costs a pass over the particles after each
  step that moved them.
- **Masks.** A `mask` collider turns the image into a field of distances, at most 16 384 cells over
  the canvas (the image itself when it is smaller), and pushes particles out of the opaque part
  along it, at the place where they went in. It works on world and local systems alike, in pixels
  of the canvas, and reads the image once when the scene loads.
- **Limits.** A scene simulates at most 64 systems, counting children and layers with particles of
  their own, and their capacities share the 200 000 particles. `prewarm` runs in steps of 1/15 s,
  at most 30 million particle steps for the whole scene, and stops once it takes more than 200 ms.
  A child never starts more particles in a step than it has room for.

A `particles` layer may also set `instance` (`alpha`, `size`, `count`, `speed`, `life`, `time_scale`,
`brightness`, `color` and `points`, which may be bound and multiply or move what the system does for
that layer only, unless the system locks them) and a `render` of its own that replaces the one of
the system (`particles2`). Each end of the `init` ranges of numbers (`life`, `speed`, `angle`,
`size`, `rotation`, `spin`, `frame`, `alpha`, `radial`, `depth`, `depth_speed`) and the numbers of
emitter shapes (`radius` and `inner` of `circle`, `threshold` of `layer`, `arc`, `radius` and
`speed` of `around`, `span` of `between`) may be bound like any value, for instance
`"size": [2, { "audio": "bass", "scale": 8 }]` (`particle_values`), and are read every frame and
kept within the limits of the field; colors, velocities and the `threshold` of `mask`, read when the
scene loads, stay literal. Births may also follow the sound or the pointer through the `rate` of the emitter, the
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
`mode: all` lets web pages list every file of the folder (`files`, `onFiles`, `web.md`), where
`one` only gives them one file at a time (`randomFile`); scene scripts list it with `folder(prop)`
(see "Scripts"), which needs the feature `script_folders`.

A `shortcut` holds what the user chose to open from the wallpaper: an `https://` address of at
most 2048 characters (opened through the OpenURI portal of the desktop, which may ask first), a
file or a folder (chosen through the file chooser and opened through the same portal, which gets
it as an open file) or an application by its desktop id (`[A-Za-z0-9._-]`, at most 255 characters),
never a command. Only the user sets it (`SetSceneProperty`, `wprs-ctl prop set open
'{"uri": "https://example.org"}'`, or the app's control under *Scene settings*), never the author, a
preset or sync, and `SetSceneProperty` refuses a file or folder that does not exist. It stays in
`config.toml` on this computer. An application is looked up as `applications/<id>.desktop` in the
data folders of the desktop: one that is hidden or runs in a terminal is refused, a `DBusActivatable`
one is activated through `org.freedesktop.Application`, and any other is started from its `Exec`
line without a shell and without its field codes; applications open only outside the Flatpak. The
content never sees the target, only `true` when one is set and `null` otherwise: a script calls `openShortcut(prop)` inside the handler of a real
`click` of a layer and a page calls `wprs.openShortcut(prop)`; the daemon opens it only within 2 s
of a click that reached the scene or page on a screen that takes clicks, for a package that holds
the `clicks` grant, at most once a second per screen, and only for a `shortcut` property of the
package shown there. The phone plays such packages without opening shortcuts. `quality` maps
quality levels (`q0` to `q3`, `q3` the highest) to a value of the property: on that level and the
lower ones the value is at most that one for a slider and exactly that one for a combo or a flag,
as the scene, its scripts and web pages see it (the stored value does not change), so a
power-saving mode turns itself on
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
`mode: all` in web pages and `script_folders` in scenes, `shortcut` and `quality_settings`; `package.md` says which platforms run each). The app shows these forms under *Scene settings* for each screen on its
Settings page, and the Android app on the page of the scene. The scenes in the `scenes/` folder of
the repository are examples of this format, dedicated to the public domain.

## Inputs

Audio analysis, the pointer and script outputs reach the renderer as plain structures of
`wprs-scene`: `AudioFrame` (64 bands per channel and their average, `bass`, `mid`, `treble`,
`level`, `beat`, `silent`; all zero while audio is off), `PointerState` (position, smoothed
position, velocity, buttons, `inside`, in fractions of the screen) and `ScriptOutputs` (values of
script sources, layer and effect overrides, copies of templates and their order, timeline,
particle and sound commands, the next timer, console lines and stopped modules), with the presses
(`Press`: a place and the buttons held after it, at most 16 between two frames) that the scene
turns into pointer events of its `solid` layers (`Touch`). A scene only renders again for an input it
uses. The pointer follows the reported position with a critically damped spring that settles in
about 0.15 s and returns to the center when the pointer leaves the screen. Audio and the pointer
need the user's grant for the scene (`dbus.md`, `Inputs1`), which the app and the tray ask for once
when such a scene is shown; a `wallpaper-rs.wpak/2` manifest declares them in `inputs`
(`package.md`), and a scene of such a package receives only the inputs its manifest declares. What
the media player plays (`media` values, cover assets, `media:` triggers and scripts that declare
it) is the `media` input, and clicks on `solid` layers the `clicks` input. Clicks are off on every
screen until the user turns them on for that screen as well as granting them to the package, they
come only from the compositor or the desktop (never from another program), and keys never reach a
scene. On phones a tap on the home screen is a click, with the package's *React to taps* switch.

## Scripts

`scripts` lists at most 64 JavaScript modules of the package (`{ "path": "scripts/spin.js" }`; more
than 16 name `scripts2`), each path once, at most 1 MiB together: `pack build`, every reader and the
engine refuse more. On the computer all the modules of a scene share one process, also when several
screens show the scene with the same settings, and on phones one thread: it has no files, network or other processes, 32 MiB of memory, a
256 KiB stack, 100 ms to load and to run the first frame, and 4 ms of CPU per frame for all the
modules together. Results reach the next frame, so a slow script never holds up rendering, and what
a frame sends to the scripts is written in place: numbers and the 64 bands of each channel go as
they are, the user settings only when they change. What the scripts send back is not compared as
text: each layer field, effect setting, particle or sound control a script writes is compared with
the value it holds, and only the parts that changed (single layers, or the values the modules
returned, compared in depth) go back, so a frame in which nothing changed sends nothing; after a
frame that failed everything is sent again. Modules that declare something (see "Script
declarations") get that frame object itself; a module that declares nothing gets a new copy of it
on every call, with plain arrays for the bands, as engines always gave it.

### Lifecycle

A module may export these functions; the engine calls each one inside the time of a frame. A module
that declares nothing only gets `update` and `timelineEvent`:

| Export | When |
|--------|------|
| `init(ctx)` | once, at the first frame, with `ctx.params` (the defaults of the module's `params`) and `ctx.engine` |
| `update(frame)` | every frame while the screen is shown; what it returns is the value of `{ "script": "<its path>" }` sources. `frame` has `time`, `dt`, `props`, `audio`, `cursor`, `actions`, `wall`, `zone`, `timelines` and `events` |
| `destroy()` | when the scene stops |
| `resize({ screen, canvas })` | when the size or the turn of the screen changes |
| `properties(changed)` | at the first frame with every user setting, then with the ones that changed |
| `settings({ language, fps, quality })` | at the first frame, then when the frame rate or the quality changes |
| `timelineEvent(e)` | for each `{ timeline, name, t }` of timeline and clip events |

A scene whose modules export no `update` and bind no value with `fn` to an `update` only runs its
scripts for events: timers, settings, properties, timeline events and pointer events, and draws no
frame for them otherwise. Once the results of busy scripts stay the same for 30 frames and nothing
else moves, the scene runs its scripts only twice a second, or at its next timer, until a result
changes again.

A value `{ "script", "fn", "params" }` binds one value to one function of a module, so a module serves
many values. `fn` names an export other than the functions above: a function called every frame as
`fn(value, ctx)`, or an object with `init(value, ctx)` (first frame) and `update(value, ctx)` (every
frame). `value` is what it returned last time (`null` at first), and `ctx` has `path` (the JSON
Pointer of the value in `scene.json`), `layer` (the id of the layer that holds it), `key` and
`params` (the module's `params` with the binding's own on top). What it returns is the value, added to
`base` and shaped like any source. `params` without `fn` is refused.

```js
import { scene, audio, setInterval, console } from "wprs";
import { Vec2, smoothstep } from "wprs/math";

const bands = audio.buffers(32);
const bars = [];
export function init({ params }) {
    for (let i = 0; i < 32; i++) bars.push(scene.spawn("bar", { position: [40 + i * 60, 900] }));
    setInterval(() => console.log("loudest band", bands.average.indexOf(Math.max(...bands.average))), 5000);
}
export function update() {
    bars.forEach((bar, i) => (bar.scale = [1, 0.1 + smoothstep(0, 1, bands.average[i]) * 4]));
}
export const sway = (value, { params }) => Math.sin(scene.layer("tree").rotation + params.speed);
```

### The `wprs` module

| Part | What it gives |
|------|---------------|
| `engine` | `time`, `dt`, `daytime` (0 at local midnight to almost 1), `fps`, `quality` (0 to 3), `locale` and `language` (the language picked among the package's languages, as for its texts, in lowercase, or `""`), `canvas` and `screen` (`[w, h]`, without names of outputs or monitors), `device` (`desktop` or `mobile`), `mode` (`wallpaper`, `lockscreen`, `screensaver`, `editor` or `preview`) and `orientation` (`landscape` or `portrait`) |
| `time` | `format(pattern, date)`: the date (`date` or now) with the tokens of `date` values in the language of `engine.locale`, such as `time.format("EEEE d 'de' MMMM")`; `Date` uses the local time zone of the computer, also after it changes, without reading any file |
| `props` | `get(id)` (a chosen file or folder reads `"wpak-user://<id>"`, never its path, and `null` while none is chosen), `all()`, `label(id)` (the label of the chosen option of a combo in that language), `color(id)` (`{ r, g, b, a }` from 0 to 1) and `pressed(id)` (a button was pressed since the last frame); scripts never write settings |
| `audio` | `bass`, `mid`, `treble`, `level`, `beat`, `bands(16 \| 32 \| 64, "left" \| "right" \| "average")` (a new array) and `buffers(16 \| 32 \| 64)`: `{ left, right, average }` as `Float32Array`s that the engine fills in place before each frame |
| `input` | `cursor`: `x`, `y` (fractions of the screen), `inside`, `buttons`, `left`, `scene` (canvas pixels) and `screen` (screen pixels); `buttons` and `left` stay 0 without the `clicks` input |
| `sound(id)` | a sound layer of the scene (`uses: ["sounds"]`, `script_sounds`): `play()` (from where `pause()` left it, or from the start), `pause()`, `stop()` (back to the start), `playing` and `volume` (0 to 1, multiplied by the volume of the layer and the user's). The first call takes the sound over from its `play_on`; the layer's own `play` and the user's mute still silence it. Phones play no scene sounds, so there the calls change nothing |
| `scene` | `layer(id)`, `layers()` (the ids of the top-level layers in drawing order), `spawn(template, options)`, `destroy(id)` and `order(id, index)`, and `mesh(pool)`, `camera`, `parallax`, `bloom` and `background` (see below) |
| `storage` | `get`, `set`, `delete`, `clear` and `keys` of what the scripts keep (see below) |
| `folder(prop)` | the pictures of a folder the user chose (see below) |
| `timeline(id)` | `play()`, `pause()`, `reverse()`, `stop()`, `seek(t)`, `speed` (−100 to 100; below zero the timeline runs backwards like after `reverse()`, and one that plays `once` ends at its start), `time`, `duration`, `playing` and `on(name, handler)` for its events and `ended` |
| `particles(layer)` | `play`, `pause` (no births, particles keep moving), `stop` (removes them until `play`), `playing`, `emit(n)` (1 to 1000), `point(n, [x, y])` and the `instance` fields `alpha`, `size`, `count`, `speed`, `life`, `time_scale`, `brightness` and `color`, which last until changed |
| `shared` | an object shared by all the modules of the scene: at most 256 values, each a frozen copy of what was set (numbers, strings, booleans, arrays, typed arrays and objects of them, at most 65536 items and 32 levels in all), so a value changes by setting it again; getters, functions and other prototypes are refused; modules may also `import` each other with relative paths (`./lib/bands.js`), among the files of `scripts` |
| `setTimeout(fn, ms)`, `setInterval(fn, ms)` | timers on the scene clock: they return a function that cancels them (`clearTimeout` and `clearInterval` call it), run at the start of a frame, at most once a frame each, without making up for missed periods and never under 16 ms; at most 64 wait at once, they wait while the scene is paused, and the scene sleeps until the next one |
| `console` | `log`, `info`, `warn`, `error` and `debug`: at most 20 lines a second of at most 512 characters, to the log of the engine at debug level and to the editor |

`scene.layer(id)` returns the same handle every time; the first 64 layers a script reads, or asks
for while it loads or in `init`, report their evaluated values every frame (a layer read for the
first time later reads what the script wrote until the next frame). Reading `position`, `scale`, `rotation`, `skew`, `origin`,
`opacity`, `visible`, `color`, `parallax`, `size` (the box of pictures, videos, solids and spectra,
the points of a text), `text`, `font` or `align` gives what the script wrote, or else the value the
scene evaluated at the last frame; `id`, `name`, `type` and `initial` (the layer as `scene.json` writes
it, its children by id) describe it. Writing those fields lasts until the script writes them again or
calls `reset(field)` (`reset()` clears them all); vectors take arrays, `{ x, y }` or `wprs/math`
vectors, colors also `{ r, g, b, a }` or `"#rrggbb"`, and `scale` and `parallax` a single number. The
engine bounds what it receives: positions and sizes to 100000 pixels, scales to 1000, `skew` to
±89°, `origin` and `parallax` to ±10, `opacity` and colors to 0..1, texts to 4096 characters and their
points to 1..2048, and fonts to the fonts the scene names or `sans-serif`, `serif` and `monospace`.
`layer.on(event, handler)` handles the pointer events of the layer (see the declarations): each
handler gets `{ type, layer, local, scene, bone }`, `local` in pixels of the layer from its top left corner
(of the group for a solid group), `scene` in canvas pixels and `bone` the touched bone of a `mesh`
layer that lists `hit` bones (else `null`). `enter`, `leave` and `move` follow
the topmost solid layer under the pointer; `down` goes to that layer, which then takes `move` and
`up` until every button is released, also off it, and `click` follows an `up` on the layer that took
the `down`. They reach the scripts in the next frame, at most 64 a frame.

`layer.video` controls the video the layer shows (`uses: ["animations"]`): `play()` (from `start`
once it ended), `pause()`, `stop()` (back to `start`, paused), `seek(seconds)` (at most one jump a
frame: the last `seek` of a frame counts), `time`, `duration` (`undefined` while it is not known),
`playing`, `rate` (0 to 16, read and written) and `on("ended", handler)`, called with `{ layer,
name }` in the frame after a video without `loop` reaches its end. Calls take the video over from the
`playback` of the layer until the scene starts again.

`layer.anim(clip)` controls the animation layers of a `mesh` layer that play the clip with that id
(`uses: ["animations"]`): `play()` (from the start once a `once` clip ended, fading in over its
`blend_in`), `pause()`, `stop()` (back to the start, fading out over its `blend_out`), `seek(seconds)`
(within the clip), `rate` (0 to 16), `weight` (0 to 1, the last one the script set, 1 before), `time`,
`duration`, `playing` and `on("ended", handler)`, called with `{ layer, clip, name }` in the frame
after a `once` clip ends. A clip that no animation layer plays gets a replacing one, at weight 1,
the first time a script calls it. Calls take the layer over from its `rate`, `weight` and
`enabled` until the scene starts again; animations run before the scripts of the next frame.

Bones, blend shapes and attachments of a `mesh` layer take `uses: ["animations"]` too. `layer.bones`
lists its bones (every node of its file) and `layer.bone(name)` returns a handle with `name`,
`parent`, `rotation` (degrees clockwise), `position` (`[x, y]` pixels, y down, in the space of its
parent) and `scale`, measured from the rest pose like the keys of clips, and `worldPosition` (pixels
of the box of the layer from its top left) and `worldRotation`; reading gives what the script set
or the pose of the last frame. Writing sets the bone for the next frame after the animations, rules
and chains and before the bodies of `physics`, so a bone with a body springs back once let go;
writing `worldPosition` or `worldRotation` replaces the local `position` and `rotation` and the other
way round, and `reset()` lets the bone go. `layer.shape(name, weight)` sets a blend shape or
expression (−1 to 1) instead of what `shapes` gives it and `layer.shape(name)` reads what the script
set; `layer.impulse(bone, [vx, vy])` pushes the end of a bone that has a body by that speed in pixels
a second, once. `layer.hit(x, y)` names the bone nearest to that point of the box (among `hit` when
it lists any) and `layer.attachment(name)` gives `{ matrix, origin, angle, uv }` of a bone or node:
its frame in canvas pixels (`[a, b, c, d, e, f]`), where it is, its angle in degrees clockwise and
its place as fractions of the box. A layer that reads bones reports their pose every frame, at
most 256 of them; the engine bounds positions to 100000 pixels, scales to 1000 and impulses to 16 a
frame.

`layer.effect(index or name)` controls one effect of the layer: `enabled`, `set(parameter, value)`
(numbers, vectors, colors, switches and blend modes, checked against the effect's description and
clamped to its range; pictures cannot be changed), `get(parameter)`, `reset()` (clears what an effect
with `state` keeps) and `index`, `name` and `effect`. What a script sets lasts until changed, and the
inputs of the scene count every effect of a scene whose scripts use `effects` as shown.

### Parents, the camera, shapes, storage and folders

With `hierarchy`, `layer.parent` is the id of the layer it follows (its `parent`, or the group that
holds it) or `null`, `layer.children` the ids of the layers that follow it or that its group holds,
and `layer.world` a `wprs/math` `Mat3` from the pixels of the layer to canvas pixels, without the
camera, as the scene evaluated it at the last frame. `layer.setParent(id, keep)` makes a layer follow
another layer of the scene (`null` to follow none) without changing the order of drawing, as `parent`
does in `scene.json`; with `keep` the layer stays where it is, its position, rotation and scale
turned into ones relative to the new parent. A layer cannot follow itself or one that follows it, nor
more than 8 parents deep: the call throws and the engine drops such a parent too. `lookAt(from, to)`
of `wprs/math` gives the angle in degrees that turns a layer at `from` towards `to`.

With `camera`, `scene.camera` (`{ center, zoom, rotation, shake: { enabled, amplitude, roughness,
speed } }`), `scene.parallax` (`{ amount, delay }`), `scene.bloom` (`{ threshold, strength, radius,
tint }`) and `scene.background` (a color) read what the scene sets in `canvas` and what the script
wrote; writing an object changes only the fields it names, within the ranges of `canvas`, and `null`
gives the field back to the scene. The camera is the same transformation as `canvas.camera`, without
passes, its shake stays still with *reduce motion* and at Q0, and bloom changes only a scene whose
`canvas` sets `bloom`.

With `geometry`, `scene.mesh(pool)` gives a pool of `pools`: `create({ vertices, indices, format,
visible })` adds a shape and returns it, with `update(fields)`, `destroy()`, `id` and `alive`;
`shapes` lists them and `clear()` removes them. `vertices` is a `Float32Array` or an array of numbers
in pixels of the pool, by vertex `x, y` (`format: "xy"`), `x, y, u, v` (`"xyuv"`, the default), `x, y,
r, g, b, a` (`"xyrgba"`) or `x, y, u, v, r, g, b, a` (`"xyuvrgba"`), colors from 0 to 1 that tint
`texture` (white without one); `indices` (a `Uint32Array` or numbers, three a triangle, at most six a
vertex) default to the vertices in order. A pool holds at most `max` shapes and `max_vertices`
vertices, the shapes of a frame take at most 1 MiB, and the engine drops a pool whose shapes name a
vertex that is not there or a number that is not finite. The shapes are drawn into a picture of the
pool of `size` (`[w, h]`, 1 to 4096, the canvas by default) that a `video` layer shows with `"video":
"<pool id>"`, so they take the place, transform, effects, masks and blending of that layer; a pool
that does not change draws nothing again and asks for no frames, and what the scripts send is binary,
not text.

With `storage`, `storage.get(key, scope)`, `set(key, value, scope)`, `delete(key, scope)`,
`clear(scope)` and `keys(scope)` keep JSON values for the package (`"package"`, the default) or for
the screen (`"output"`), at most 64 KiB in each scope: `set` throws past that or for a value that is
not JSON. They are kept on the device only (never synced or exported), in a file of the package
(mode 0600) that is written at most every 5 s and when the scene stops, and removed with the package;
a scene whose scripts use storage runs one script process per screen. The screens showing a package
share its `package` scope: what one stores reaches the scripts of the others on their next frame,
and each keeps its own `output` scope (at most 16 screens, the one used longest ago going first).
Previews in the editor read what is kept but never write it.

With `folders`, `folder(prop)` lists a `folder` property with `mode: all`: `files` are opaque ids of
its pictures (an id changes when its file is written again), `name(id)` the name of the file, and
`on("added" | "removed" | "changed", handler)` gets `{ prop, files }` (`changed` as `[old, new]`
pairs) when the folder changes, looked at every 2 s while the scene is shown. `show(layer, id)` makes
an image layer show that file instead of its own picture (`null` gives it back), at most eight files
at once. Scripts never see paths, and the phone lists the folder the user chose in the same way.

`templates` maps at most 64 ids to layers that scripts with `layers` copy into the scene, validated
like the other layers (`script_layers`); a template id may not be the id of a layer, a template holds
at most 256 layers with its children, and templates are pictures, texts, shapes, solids, spectra and
groups of them. `scene.spawn(template, options)` adds a
copy on top of the layers (or at `options.at`, 0 being the bottom) with the id `options.id` (letters,
digits, `_` and `-`, at most 64) or `<template>_<n>`, sets the fields `options` names and returns its
handle; the children of a copied group are `<id>.<child>`. `destroy()` removes it, `scene.order(id,
index)` moves a top-level layer, and at most 256 copies live at once, and only while the layers of
the scene and of every copy, children included, are at most 512: `spawn` throws past that and the
engine drops what goes beyond it. Copies count as layers of the scene for the inputs it uses and for
its cost, and are drawn with the same resources as the other layers: pictures share the texture of
their asset and equal texts one raster.

The `wprs/math` module, written for this engine, has `Vec2`, `Vec3` and `Vec4` (`add`, `sub`, `mul`,
`div`, `min`, `max`, `clamp`, `scale`, `negate`, `abs`, `floor`, `ceil`, `round`, `dot`, `length`,
`lengthSquared`, `normalize`, `distance`, `mix`, `lerp`, `reflect`, `project`, `equals`, `isZero`,
`toArray` and, by size, `cross`, `angle`, `rotate` and `perpendicular`), `Mat3` (2D, with
`translation`, `rotation`, `scaling`, `transformPoint`, `transformVector` and `decompose`) and `Mat4`
(`translation`, `scaling`, `rotationX/Y/Z`, `fromEuler`, `perspective`, `lookAt`, `transformPoint`,
`transformVector` and `decompose`), both with `multiply`, `transpose`, `determinant`, `invert`,
`equals` and `get(row, col)`, and `clamp`, `mix`, `smoothstep`, `step`, `fract`, `remap`, `degrees`,
`radians`, `angleDelta`, `mixAngle`, `angleVector`, `vectorAngle`, `lookAt`, `rgbToHsv` and `hsvToRgb` (hue
from 0 to 1). Vectors and colors go in and out as these classes or as arrays.

`Math.random` follows `clock.seed`, and `clock.seed: null` gives it a new seed on every start.
Scripts never see paths, device names, the battery or the list of fonts.

### When a script fails

A function that throws writes the error to the console and the frame goes on with the other modules;
a module whose functions throw in three frames in a row stops alone, with its timers, and the
screen reports "A script of this scene stopped and the scene goes on without it: <module>: <error>"
in the language of the desktop, as does `wprs-ctl status`. A module that does not load stops the same
way. A frame that runs out of time or memory fails as a whole, and after three such frames in a row
the scripts of the scene are off and the screen says so; the scene keeps drawing without them. On
phones the scripts run in a thread of their own with the same limits, and the diagnostics of the app
say when a module stopped or the scripts are off.

### Script declarations

Each entry of `scripts` may declare what its module needs from the engine, so that an engine without
it refuses the package instead of running the module wrongly, and so that its inputs are known
without reading its text. `wprs-ctl scene import-script` and the editor propose them from the
module's exports, imports and calls.

| Field | Meaning |
|-------|---------|
| `events` | the functions the module exports and the events it handles, each once: the lifecycle above (`update` is the only one older engines call), the pointer events of `solid` layers and `hit` bones `click`, `down`, `up` (input `clicks`), `enter`, `leave` and `move` (input `pointer`), delivered with `scene.layer(id).on(event, handler)` as `{ type, layer, local, scene }` with the touched bone, and `mediaStatus`, `mediaPlayback`, `mediaProperties`, `mediaThumbnail` and `mediaTimeline` (input `media`) |
| `uses` | the parts of the `wprs` module it uses, each once: `audio`, `pointer` and `clicks` (those inputs), `timers`, `layers` (`spawn`, `destroy` and `order`; `script_layers`), `effects`, `particles` (it names `particles2` and gives each particles layer particles of its own), `math`, `console`, `shared`, `media` (the `media` object and its colors, input `media`; `media_info`, which engines run later), `storage` (`storage`; `script_storage`), `geometry` (`scene.mesh(pool)`; needs `pools`; `script_geometry`), `sounds` (`sound(id)` with `play`, `pause`, `stop`, `playing` and `volume`; `script_sounds`), `animations` (`layer.video`, `layer.anim(clip)` and, on `mesh` layers, `layer.bone(name)`, `bones`, `shape`, `impulse`, `hit` and `attachment`; texture frames and model clips come with the layers that have them; `script_animations`), `camera` (`scene.camera`, `parallax`, `bloom` and `background`; `script_camera`), `shortcuts` (`openShortcut(prop)`, which asks to open what the user chose for that `shortcut` property and returns `true` only inside the handler of a `click`, at most four properties a frame; see "User properties and presets"; `script_shortcuts`), `hierarchy` (`layer.parent`, `layer.children`, `layer.world` and `layer.setParent(id, keep)`; `script_hierarchy`) and `folders` (`folder(prop)`; `script_folders`), described in "Parents, the camera, shapes, storage and folders" |
| `params` | at most 32 settings of the module `{ "type": "number" \| "bool" \| "text" \| "color" \| "vec2", "default", "min", "max" }` that values bound to it with `fn` and `params` fill in; ids `[a-z0-9_]{1,32}`. They live only here: the module does not export them and `scene import-script` does not propose them |

Any event but `update`, any part but `audio`, `pointer` and `clicks`, and `params` name `scripts2`;
handling pointer events needs a `solid` layer or `hit` bones. A module that declares something must
declare `timers`, `layers` and `effects` to use them: the engine refuses the call otherwise. A module
that declares nothing, and that no value binds with `fn` or `params`, gets the first interface, the
same on every engine: `engine` (`time`, `dt`, `locale`, `daytime`), `time`, `props` (`get`, `label`,
`pressed`, `color`), `audio` (`bands` and the levels), `input.cursor` (`x`, `y`, `inside`,
`buttons`), `scene.layer(id)` with `position`, `scale`, `rotation`, `opacity`, `visible`, `color`
and `text` (a write lasts for the frame it is made in, as engines always did, and reading gives
what the module wrote in that frame), `timeline` and `particles`, and only `update` and
`timelineEvent` are called. `pack build`, `pack check` and the editor refuse such a module when it
imports anything else from `wprs`, imports `wprs/math` or another module, uses `console` or the
timers, or exports `init`, `destroy`, `resize`, `properties` or `settings`, and the engine refuses
those imports and calls. Its inputs are those of what it imports from `wprs`: `audio` the sound and
`input` the pointer, never words that only appear in its comments or texts. Effect uses may carry a `name` (`[a-z0-9_]{1,32}`, unique in their layer) by which scripts find
them (`scripts2`). `pools` maps at most 16 ids to `{ "type": "mesh", "max" (1 to 256 shapes),
"max_vertices" (3 to 65536), "texture", "size" }`, the shapes scripts with `geometry` may build
(`script_geometry`).

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
| an effect masked by another layer or by a shape | the effect's `mask` with `layer` or `shape` |
| an effect that reads what another layer draws | a `texture` parameter set to `{ "layer" }` |
| an effect that copies what lies below the layer | a pass that reads `backdrop` |
| blend modes such as linear light, pin light, hard mix or divide | `blend` of the layer, and `blend_mode()` in shaders |
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

### Effects with an equivalent

Effects that other programs ship as built-ins become these effects. The built-in effects are written
from scratch, with the parameters written here, so an imported value is converted rather than copied.
Positions are fractions of the layer counted from the top left, angles are degrees that turn
clockwise, and sizes are pixels of the layer unless the row says otherwise; a value given as a
fraction of the picture becomes pixels when multiplied by the width of the layer, and an angle
counted counterclockwise changes its sign.

| The original effect | Built-in effect | Parameters to convert |
|---------------------|-----------------|-----------------------|
| a blur of the whole layer | `blur` | `radius` in pixels |
| a blur away from a center | `blur_radial` | `center`, `strength` (0 to 1) |
| a blur along a direction | `blur_motion` | `angle`, `length` in pixels |
| a color made transparent | `chroma_key` | `key`, `tolerance`, `softness` |
| edge detection | `edges` | `threshold`, `width` in pixels, `color` |
| a fisheye or lens bulge | `lens_distort` | `k1` (above 0 bulges), `center` |
| a four-point perspective | `perspective` | `top_left`, `top_right`, `bottom_right`, `bottom_left` |
| a shake | `shake` | `strength` in pixels, `speed`, `direction` |
| eyes that move | `iris` | `range` in pixels, `center` |
| water that flows along a painted map | `flow` | `map` (a `linear` picture), `strength` in pixels, `speed` |
| parallax from a depth map | `depth_parallax` | `depth` (a `linear` picture, bright is near), `strength` in pixels, `focus` |
| a mirrored reflection | `reflection` | `line` (from the top), `opacity` |
| drifting clouds | `clouds` | `size` and `speed` in pixels, `coverage` |
| an opacity mask painted on the layer | the layer's `mask` | `texture`, `strength` |
| foliage that sways | `sway` | `amplitude` in pixels, `pivot`, `speed` |
| a line of light that crosses the layer again and again | `shine` | `angle`, `width` (fraction of the layer), `speed` (crossings a second), `gap` |
| rays from the bright parts of the picture | `god_rays` | `source`, `exposure`, `decay` |
| color fringes | `chromatic_aberration` | `amount` in pixels |
| film grain | `film_grain` | `amount`, `size` in pixels |
| a twirl | `twirl` | `angle`, `radius` in pixels, `center` |
| a picture that scrolls | `scroll` | `speed` in layers a second |
| water waves | `waves` | `amplitude` in pixels, `frequency` (waves across the layer), `speed`, `direction` |
| ripples where the pointer touches water | `water_ripple` | `strength`, `radius` in pixels |
| a tint with a blend mode | `tint` | `color`, `blend`, `amount` |
| up to six pictures or layers blended with modes | `blend_texture`, one use each | `texture` (an asset or `{ "layer" }`), `blend`, `amount` |
| animated beams of light | `light_shafts` | `direction`, `angle`, `center`, `color`, `gradient`, `exponent`, `intensity`, `feather`, `smoothness`, `speed` |

A test reads this table and checks that every effect and parameter in it exists.

### Porting a shader to WGSL

Package shaders are written in WGSL and nothing else: the engine never compiles GLSL or HLSL. An
effect of another program becomes one package effect: its passes, render targets and copies become
`passes`, `targets`, `copy` and `swap`, its compile-time switches `options`, its uniforms `params`,
and its fragment and vertex programs entry points of one `.wgsl` file. The rules of thumb:

- Write one `@fragment` function per pass, `fn fs_<name>(in: VertexOut) -> @location(0)
  vec4<f32>`, and return the color instead of writing `gl_FragColor` or an `out` variable. Colors
  are premultiplied by alpha; `straight(c)` undoes it.
- Coordinates go from (0, 0) at the top left to (1, 1) at the bottom right, in `in.uv`. Flip `y`
  where the original counts from the bottom (`1.0 - uv.y`).
- Move what the original computes per vertex but is affine (offsets, scales, rotations of the
  coordinates) into the fragment entry point; keep a `vertex` entry with a `grid` only to bend the
  layer itself.
- Turn every `#ifdef`, `#if` or combo into an `override` with `options`, or into an `if` on a
  parameter when it changes at run time, and every texture slot into an `inputs` entry and a
  `texture` parameter.
- Give every loop a literal limit: `for (var i = 0u; i < min(count, 32u); i++)`.

| HLSL or GLSL | WGSL |
|--------------|------|
| `float`, `float2`, `vec3`, `half4`, `int2`, `bool` | `f32`, `vec2<f32>`, `vec3<f32>`, `vec4<f32>`, `vec2<i32>`, `bool` |
| `float2x2`, `mat3` | `mat2x2<f32>`, `mat3x3<f32>` |
| `mul(m, v)` in HLSL, `m * v` in GLSL | `v * m` for an HLSL matrix written by rows, `m * v` for a GLSL one |
| `frac(x)` | `fract(x)` |
| `saturate(x)` | `clamp(x, 0.0, 1.0)` |
| `lerp(a, b, t)` | `mix(a, b, t)` |
| `fmod(a, b)` (HLSL, truncated) | `a - b * trunc(a / b)` |
| `mod(a, b)` (GLSL, floored) | `a - b * floor(a / b)` |
| `atan2(y, x)`, GLSL `atan(y, x)` | `atan2(y, x)` |
| `rsqrt`, `inversesqrt` | `inverseSqrt` |
| `ddx`, `ddy`, `dFdx`, `dFdy` | `dpdx`, `dpdy` |
| `tex2D(s, uv)`, `texture(s, uv)`, `texture2D` | `sample(aux0, uv)`, or `tap(uv)` for the layer itself |
| `tex2Dlod`, `textureLod` | `textureSampleLevel(t, samp, uv, level)` |
| `discard` / `clip(x)` | `discard` / `if x < 0.0 { discard; }` |
| `static const`, `#define N 8` | `const N: u32 = 8u;` |
| a uniform shown as a slider, color or point | a `float`, `color` or `position` parameter read with `param(n)` |
| a combo or `#define` chosen before compiling | an `override` named by `options` |
| the time and the time since the last frame | `frame.time`, `frame.dt` |
| the time of day from 0 to 1 | `daytime()` |
| the pointer and its previous position | `frame.cursor`, `cursor_last()`, or `cursor_uv()` on the layer |
| the size of the screen and of a texture | `frame.resolution`, `tex_size(i)` and `fx.texel` |
| 16, 32 or 64 bands of sound, per channel | `band_of(i, 16u)`, `band_left_of(i, 16u)`, `band_right_of(i, 32u)`, `band_left(i)`, `band_right(i)` |
| the parallax shift | `parallax()` |
| the layer's color and opacity | `layer_color()`, `layer_alpha()` |
| greyscale, HSV, 2D rotation and blending helpers | `greyscale`, `hsv`, `from_hsv`, `rotate2`, `blend_mode` |

Package shaders declare no bindings of their own: the input, the mask and up to eight more textures
come through `inputs` and `params`. `wprs-ctl scene import-effect` checks the result the same way a
package reader does and says what to change.
