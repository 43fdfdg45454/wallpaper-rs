# D-Bus interface

Every component takes these names from `wprs_core::dbus`.

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
- A request with the same size returns the existing stream and does not re-render.
- A request with a different size replaces the stream; the old serial becomes invalid.
- Sizes must be in `1..=min(16384, GPU limit)`; otherwise `InvalidArgs`.

Stream format: raw video, `BGRx` (or `BGRA`), fixed size, variable frame rate, memfd buffers.
Frames are produced when the picture changes, when a consumer starts streaming, and every
10 s as a keep-alive.

### `SetWallpaper(s path, s output, s fit)`

Sets the image for `output`, or for every output when `output` is empty (this also clears
per-output images). `fit` is `fill`, `fit`, `stretch`, `center`, `tile`, or empty to keep the
current mode. A `file://` prefix is accepted. The image is decoded before the configuration is
saved; failures return `Failed` and change nothing.

### `Reload()`

Re-reads the configuration file and re-renders every output.

### `GetState() → s`

JSON document for diagnostics:
`{"version", "adapter", "config", "outputs": {name: {"width", "height", "image", "fit"}}}`.

## Properties

| Name | Type | Meaning |
|------|------|---------|
| `Version` | `s` | Daemon version |
| `Adapter` | `s` | GPU adapter and backend in use |
