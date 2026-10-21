# D-Bus interface

Every component takes these names from `wprs_core::dbus`. The command line client
(`wprs-ctl`) and the management interface in the system tray are both plain clients of this
interface: every action is available in both.

| Item | Value |
|------|-------|
| Bus | session |
| Well-known name | `io.github.wallpaper_rs.Daemon` |
| Object path | `/io/github/wallpaper_rs/Daemon` |
| Interface | `io.github.wallpaper_rs.Daemon1` |

The daemon is D-Bus activatable (`packaging/dbus`) and runs as the systemd user unit
`wprsd.service`.

## Methods

### `RequestStream(s output, u width, u height) → (u node_id, t serial)`

Creates or reuses the PipeWire video stream for `output` at `width` × `height` physical pixels
and returns the node id and `object.serial` of the stream node. Consumers should connect by
serial; `serial` is `0` only if PipeWire did not publish one within 2 s, in which case the node
id can be used.

- `output`: connector name such as `DP-1`. Empty means `default`.
- A request with the same size returns the existing stream.
- A request with a different size replaces the stream; the old serial becomes invalid.
- Sizes must be in `1..=min(16384, GPU limit)`; otherwise `InvalidArgs`.

Stream format: raw video, `BGRx` (or `BGRA`), fixed size, variable frame rate, one memfd per
buffer mapped from offset 0. Frames are produced when the picture changes (new wallpaper,
transition, video frame), when a consumer starts streaming, and every 10 s as a keep-alive.

### `ReleaseStream(s output)`

Destroys the stream of `output` when its client no longer shows it.

### `SetWallpaper(s path, s output, s fit)`

Sets the wallpaper of `output`, or of every output when `output` is empty (this also clears
per-output wallpapers). `path` is an image or a video (`mp4`, `m4v`, `webm`, `mkv`, `mov`,
`avi`, `ogv`, `mpg`, `mpeg`); videos loop and are decoded with GStreamer. `fit` is `fill`,
`fit`, `stretch`, `center`, `tile`, or empty to keep the current mode. A `file://` prefix is
accepted. Images are decoded before the configuration is saved; failures return `Failed` and
change nothing. A new wallpaper replaces the old one with the configured transition.

### `SetOption(s key, s value)`

Changes one setting and saves the configuration. Keys and their current values are listed in
`GetState().options`:

| Key | Values |
|-----|--------|
| `playback.volume` | `0.0` to `1.0` |
| `playback.muted` | `true`, `false` |
| `playback.max_fps` | `1` to `240` |
| `transition.kind` | `fade`, `none` |
| `transition.duration_ms` | `0` to `10000` |
| `pause.when_covered` | pause while a maximized or fullscreen window covers the output |
| `pause.on_battery` | pause while running on battery |
| `pause.when_locked` | pause while the screen is locked |

Invalid keys or values return `Failed` and change nothing.

### `SetPaused(b paused)`, `SetVolume(d volume)`, `SetMuted(b muted)`

Pause or resume every animated wallpaper; set the volume and mute state of video wallpapers.

### `SetOutputCovered(s output, b covered)`

Reported by the desktop integration when a maximized or fullscreen window covers `output`.

### `Reload()`

Re-reads the configuration file and applies it.

### `GetState() → s`

JSON document: `version`, `adapter`, `config` (file path), `paused`, `on_battery`, `locked`,
`volume`, `muted`, `options` (key → value) and `outputs`, where each output has `width`,
`height`, `image`, `fit`, `kind` (`color`, `image` or `video`), `playing`, `active`, `covered`
and `error`.

## Properties

| Name | Type | Meaning |
|------|------|---------|
| `Version` | `s` | Daemon version |
| `Adapter` | `s` | GPU adapter and backend in use |
| `Paused` | `b` | Paused by the user |
| `Volume` | `d` | Volume of video wallpapers |
| `Muted` | `b` | Video wallpapers are muted |
