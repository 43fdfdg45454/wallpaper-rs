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
`wprsd.service`. In the Flatpak (`packaging/flatpak`) it owns the name through `--own-name`, so
Flatpak exports the same activation file, starts the daemon with `flatpak run` and drops
`SystemdService`; there is no systemd unit there. The name is owned without allowing
replacement. A second daemon checks the name before initializing the GPU and PipeWire and exits
at once with an error saying that another wprsd owns it.

Errors: invalid arguments return `InvalidArgs`; other failures return `Failed` and change
nothing. If the render thread has stopped after an internal error, every method that changes
something returns `Failed` until the daemon is restarted.

## Streams and claims

A stream exists while at least one claim holds it. Claims are kept per output and per client:

- A desktop client is identified by its unique bus name (the sender of the call). Each
  `RequestStream` adds one claim for that client, so one connection that shows the same output
  several times (for example, several containments) holds several claims. Each `ReleaseStream`
  removes one of them.
- The held claim belongs to no connection. `HoldStream` creates it and it survives the exit of
  the caller; there is at most one per output. `ReleaseStream` from a client without a claim on
  that output removes the held claim.
- When a client leaves the bus (`NameOwnerChanged` with an empty new owner), all its claims are
  removed, even if it never called `ReleaseStream`.
- The stream is destroyed when its last claim is removed.

Coverage is also tracked per claimant. An output counts as covered only when every claimant
reports it covered. `SetOutputCovered` from a client without a claim applies to the held claim.

Output names are trimmed and an empty name means `default` in `RequestStream`, `HoldStream`,
`ReleaseStream` and `SetOutputCovered`.

## Methods

### `RequestStream(s output, u width, u height) → (u node_id, t serial)`

Adds a claim of the caller on `output` and creates or reuses the PipeWire video stream at
`width` × `height` physical pixels. Returns the node id and `object.serial` of the stream node.
Consumers should connect by serial; `serial` is `0` only if PipeWire did not publish one within
2 s, in which case the node id can be used.

- `output`: connector name such as `DP-1`.
- A request with the same size returns the existing stream.
- A request with a different size replaces the stream; the old serial becomes invalid.
- Each edge must be in `1..=min(16384, GPU limit)` and one frame, with rows padded to 256
  bytes, must fit in 256 MiB (for example 8192 × 8192); otherwise `InvalidArgs`.
- Call `ReleaseStream` once for every successful `RequestStream`.

Stream format: raw video, `BGRx` (or `BGRA`), fixed size, variable frame rate, one memfd per
buffer mapped from offset 0. Frames are produced when the picture changes (new wallpaper,
transition, video frame), when a consumer starts streaming, and every 10 s as a keep-alive.

### `HoldStream(s output, u width, u height) → (u node_id, t serial)`

Same as `RequestStream`, but with the held claim. Used by `wprs-ctl stream` for tests and
tools that exit right after the call.

### `ReleaseStream(s output)`

Removes one claim of the caller on `output`, or the held claim if the caller has none.

### `SetWallpaper(s path, s output, s fit)`

Sets the wallpaper of `output`, or of every output when `output` is empty (this also clears
per-output wallpapers). `path` is an image or a video (`mp4`, `m4v`, `webm`, `mkv`, `mov`,
`avi`, `ogv`, `mpg`, `mpeg`); videos loop and are decoded with GStreamer. `fit` is `fill`,
`fit`, `stretch`, `center`, `tile`, or empty to keep the current mode. A `file://` prefix is
accepted. Images are decoded before the configuration is saved. A new wallpaper replaces the
old one with the configured transition.

With an empty `path`, only the fit changes and the current wallpapers stay: the fit of
`output`, or of every output when `output` is empty. A fit for every output also clears
per-output fits, with or without a path. An empty `path` and an empty `fit` return
`InvalidArgs`.

### `SetBackground(s color, s output)`

Sets the color shown around and behind the wallpaper of `output`, or of every output when
`output` is empty (this also clears per-output colors). `color` is `#rrggbb`; anything else
returns `InvalidArgs`.

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

Only the changed key is validated. Unknown keys and invalid or out-of-range values return
`InvalidArgs` and change nothing.

### `SetPaused(b paused)`, `SetVolume(d volume)`, `SetMuted(b muted)`

Pause or resume every animated wallpaper; set the volume and mute state of video wallpapers.

### `SetOutputCovered(s output, b covered)`

Reported by the desktop integration when a maximized or fullscreen window covers `output`.
See "Streams and claims".

### `Reload()`

Re-reads the configuration file and applies it. If the file is invalid, the current
configuration stays, `config_error` is set and the call returns `Failed`.

### `GetState() → s`

JSON document: `version`, `adapter`, `config` (file path), `config_error`, `paused`,
`on_battery`, `locked`, `volume`, `muted`, `options` (key → value) and `outputs`, where each
output has `width`, `height`, `image`, `fit`, `kind` (`color`, `image` or `video`), `playing`,
`active`, `covered`, `clients` (number of claimants) and `error`.

## Properties

| Name | Type | Meaning |
|------|------|---------|
| `Version` | `s` | Daemon version |
| `Adapter` | `s` | GPU adapter and backend in use |
| `Paused` | `b` | Paused by the user |
| `Volume` | `d` | Volume of video wallpapers |
| `Muted` | `b` | Video wallpapers are muted |

`PropertiesChanged` is emitted for `Paused`, `Volume` and `Muted` whenever their values change,
whichever method changed them (including `SetOption` and `Reload`).

## Configuration file

`~/.config/wallpaper-rs/config.toml` is written atomically with mode `0600`; the directory is
created with mode `0700`. If the file is a symbolic link, the link is kept and its target is
replaced. Values out of range are clamped when the file is read and invalid colors fall back to
the default color, with a warning naming the key.

If the file cannot be parsed at startup, the daemon starts with the default configuration and
reports the error in `GetState().config_error` (`null` when the file is fine). Before the first
save, the broken file is copied to `config.toml.broken` (mode `0600`).
