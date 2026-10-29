# D-Bus interface

Every component takes these names from `wprs_core::dbus`. The command line client (`wprs-ctl`)
and the desktop app are plain clients of this interface: every action is available in both. The
system tray and the desktop integration offer a subset.

| Item | Value |
|------|-------|
| Bus | session |
| Well-known name | `io.github.wallpaper_rs.Daemon` |
| Object path | `/io/github/wallpaper_rs/Daemon` |
| Interfaces | `io.github.wallpaper_rs.Daemon1`, `Library1`, `Playlists1`, `Profiles1`, `Providers1`, `Repos1`, `Inputs1`, `Sync1`, all on that object |

The daemon is D-Bus activatable (`packaging/dbus`) and runs as the systemd user unit
`wprsd.service`. In the Flatpak (`packaging/flatpak`) it owns the name through `--own-name`, so
Flatpak exports the same activation file, starts the daemon with `flatpak run` and drops
`SystemdService`; there is no systemd unit there. The name is owned without allowing
replacement. A second daemon checks the name before initializing the GPU and PipeWire and exits
at once with an error saying that another wprsd owns it.

A panic in any thread of the daemon, a lost GPU device, a GPU that stops responding or a Wayland
protocol error from the compositor makes the daemon exit with status 70; the user unit restarts
it (`Restart=on-failure`, at most 5 starts in 60 s) and clients get their streams again through
`RequestStream`. Only the methods that change what is shown (`Library1.Apply`,
`Playlists1.Start`, `Stop`, `Next` and `Previous`, `Profiles1.Activate` and the `Daemon1` methods
that change something) need the renderer: while it is not running they return `Failed` (`the
renderer stopped after an error; restart wprsd`) and change nothing. Every other method keeps
working.

The daemon opens network connections only to the providers, repositories and the sync server
the user added (including the signed mirrors of a repository, the only hosts a repository request
may be redirected to), with a generic `User-Agent:
wallpaper-rs/<version>` and without cookies or `Referer`. At startup it contacts only providers
due for their automatic sync and a linked sync server whose sync is not paused; the daily
repository refresh starts 10 minutes later.

## Conventions

- Members are PascalCase; argument names are snake_case and are part of the interface.
- Queries return one `s`: a JSON object whose first fields are `model` (the data model version,
  property `Model`) and `serial` (the value of property `Serial` read before the query ran). The
  shapes are listed in "JSON model".
- Mutations take typed arguments and return nothing or a created identifier. Everything is
  validated before anything changes: a failed call changes nothing. Calls that take several
  uids check every uid first and change them in one transaction.
- Operations that walk folders, copy files or touch the network return a job id at once and run
  in the background (see "Jobs").
- Library identifiers (`uid`) are ULIDs: 26 characters, uppercase Crockford base32. Only full
  uids travel on the bus, except in `Library1.Resolve` and where an argument is documented as
  "uid or name".
- Output arguments are connector names such as `DP-1`, trimmed; more than 128 bytes or control
  characters return `InvalidArgs`. `""` means every output. In the interfaces other than
  `Daemon1`, a name starting with `@` returns `InvalidArgs`.
- Times are `i64` Unix seconds, UTC.
- Paths are absolute and UTF-8. `SetWallpaper` also accepts `file://` URIs; every other method
  rejects URIs with `InvalidArgs`, so clients convert `file://` URIs to paths (percent-decoded)
  before calling.
- Batch arguments (`as uids`, `as paths`, `au indexes`) take at most 10 000 entries (1 000 for
  `Import`, 500 for `RequestThumbnails` and `RequestPreviews`); page `limit` is at most 500.
- Titles have 1 to 256 characters, playlist, profile and preset names 1 to 128, tags 1 to 64;
  all are trimmed and may not contain control characters.

## Errors

Every error carries a readable English message.

| Error name | When |
|------------|------|
| `org.freedesktop.DBus.Error.InvalidArgs` | Malformed or out-of-range argument, bad query syntax, operation not allowed on that object |
| `org.freedesktop.DBus.Error.Failed` | Anything else: I/O, database, verification failure, library unavailable, renderer stopped |
| `org.freedesktop.DBus.Error.NotSupported` | The method is not available in this version of wprsd |
| `io.github.wallpaper_rs.Error.NotFound` | Unknown uid, name, output assignment, provider, repository, package or job |
| `io.github.wallpaper_rs.Error.Exists` | Name, folder, provider id or repository already present |
| `io.github.wallpaper_rs.Error.NeedsSecret` | A provider needs a secret that is missing or was rejected |
| `io.github.wallpaper_rs.Error.KeyringUnavailable` | The keyring item cannot be written or deleted, the keyring stayed locked for 20 s or its unlock prompt was dismissed, the Secret Service did not answer in time, or no Secret Service is available to store a secret |
| `io.github.wallpaper_rs.Error.Stale` | A repository's signed index expired; retry with `allow_stale = true` after asking the user |
| `io.github.wallpaper_rs.Error.Conflict` | A playlist edit by index found other entries than expected: `the playlist changed; reload it` |
| `io.github.wallpaper_rs.Error.Newer` | A package needs a newer wallpaper-rs: a newer manifest version, a `requires.engine` this version does not meet or a feature of `requires.features` it does not know (see `package.md`) |
| `io.github.wallpaper_rs.Error.Unconfirmed` | `Apply` of a photosensitive package the user has not allowed yet (`this wallpaper flashes or shows fast patterns that can trigger seizures in people with photosensitive epilepsy; the user has not allowed it yet`); ask, then call `Confirm(uids, "photosensitive")` and apply again |

If `library.db` cannot be opened (for example because a newer build wrote it), the daemon still
renders, `GetState().library_error` holds the reason and every `Library1`, `Playlists1` and
`Profiles1` method, `Providers1.Add`, `Remove` and `Sync`, and `Repos1.Install`, `Uninstall`
and `Remove(uninstall = true)` return `Failed` with it; scheduled syncs are skipped. The other
`Providers1` and `Repos1` methods keep working. An output whose playlist cannot be resolved then
shows its background color and `GetState().outputs[x].error` says why.

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

## Layer-shell and X11 outputs

Outside Plasma, on Wayland compositors that offer `zwlr_layer_shell_v1`, the daemon shows each
`wl_output` itself on a background layer surface, and in X11 sessions each RandR monitor on a
desktop window below all others (`wprsd --backend auto|streams|layer-shell|x11`). These outputs
are named after the compositor's output names or the RandR monitor names and appear in
`GetState` like streams, with one claim owned by the daemon: `ReleaseStream` and
`SetOutputCovered` do not apply to them. They count as covered while the compositor stops
sending frame callbacks, while an active fullscreen or maximized window is on them, on Hyprland
while their workspace has a fullscreen window, or in X11 while a window shown on the current
desktop is fullscreen or maximized on them; `pause.when_covered` then pauses their video as for
streams. In X11, still images also become the root window background (`_XROOTPMAP_ID` and
`ESETROOT_PMAP_ID`).

An output that appears, changes or disappears is applied once it has stayed the same for 0.5 s: an
output that the compositor reports twice, removes and adds back, or announces for a moment with a
zero size keeps its wallpaper, its playback and a single surface; a layer surface without a size
is shown only once the compositor gives it one. When the compositor closes the connection the
daemon exits with status 0; a protocol error ends it with status 70 so that the user unit starts
it again. In X11, an exposed desktop window is drawn again from the last frame, so a paused video
stays paused. The desktop windows are managed by the window manager when it announces
`_NET_WM_WINDOW_TYPE_DESKTOP` in `_NET_SUPPORTED`, and are *override-redirect* windows kept at
the bottom otherwise; the daemon replaces them when a window manager starts or stops.

## Frame rate

Every output knows the refresh rate of its screen: layer-shell outputs take it from the current
mode of their `wl_output`, X11 outputs from the RandR mode of their CRTCs (the fastest when a
monitor spans several) and streams from the current mode of the `wl_output` with the same name
while the daemon is connected to the Wayland compositor, which on Plasma is KWin's own output
information. With variable refresh the mode gives the highest rate. An output never renders more
often than the lowest of:

- `playback.max_fps`: its number, or with `display` (the default) the refresh rate of the output
  on mains power (60 while it is unknown) and 30 on battery;
- a known refresh rate;
- except for videos, the GPU class: software renderers 24 (15 on battery), integrated GPUs 60 (30
  on battery) or, on mains power, the rate above for a scene whose estimated cost at that rate
  fits their budget, dedicated GPUs the rate above (60 on battery);
- for scenes, their `clock.max_fps` (`display` follows the refresh rate) and the quality level;
- for parallax images with software rendering, 15;
- for videos with `display` and software rendering, 30.

A fade between two wallpapers runs at 60 frames per second on mains power, or faster when the
rate above is higher, never above a known refresh rate; on battery it keeps the rate above.
Videos play at their own frame rate and drop the frames above the rate of the fastest output that
shows them, never repeating one. Nothing is rendered while nothing changes, whatever the rate.

## Stream buffers

Each stream offers its consumers two formats, in this order:

1. GPU memory: the raw video format with the `SPA_FORMAT_VIDEO_modifier` property, mandatory and
   not fixated, listing every DRM format modifier that the GPU can export as a single-plane
   `B8G8R8A8` color attachment. PipeWire keeps the modifiers that the consumer can also import, as
   it announces them for screen casts (KPipeWire does so when the scene graph can import
   DMA-BUFs). The daemon allocates a test image with the remaining modifiers, lets the driver
   choose one and announces the format again with that modifier fixed. The buffers are then
   `SPA_DATA_DmaBuf`, one plane each, with the offset and stride of the plane in their chunk.
2. Shared memory: the same format without a modifier, one memfd per buffer mapped from offset 0.

With GPU memory, each frame is drawn straight into one of the 4 to 8 stream buffers: the CPU
neither reads it back nor copies it, and a video frame is uploaded to the GPU once and leaves in
the exported buffer. A buffer is drawn into only after the consumer gave it back, the one sent
longest ago first, and never while it holds the last frame, which keep-alives send again when the
consumer has given it back. The daemon releases the image to external use and waits for the GPU to
finish the frame before it queues the buffer. When the consumer still holds every other buffer,
the frame is skipped and tried again one frame later.

The stream uses shared memory, without any action from the consumer, when:

- the renderer is not Vulkan (the GLES backend), or the driver cannot export such images through
  `VK_KHR_external_memory_fd` and `VK_EXT_external_memory_dma_buf` (without
  `VK_EXT_image_drm_format_modifier` only linear images, modifier `0`, are offered);
- rendering is in software, unless the daemon runs with `WPRS_DMABUF=1`, or the daemon runs with
  `WPRS_DMABUF=0`;
- the consumer does not offer DMA-BUFs or none of its modifiers is exportable;
- a buffer cannot be allocated. The daemon allocates the first four buffers of a stream as soon as
  the consumer has chosen their layout, before any is handed out, so a failure there gives that
  consumer shared memory; a failure for a fifth or later buffer recreates the stream in shared
  memory. The stream then offers shared memory only, and new streams of that output do too for a
  minute.

GPU memory stays in use on battery, since it only removes work. All consumers of one stream share
its buffers: the first consumer decides their kind, a consumer that links later receives the same
buffers, and the kind is chosen again once all consumers have left. `GetState` shows the kind of
buffers of each output in `buffers`. Debug builds also accept `WPRS_DMABUF=emulate`, which
exports the Vulkan memory of software rendering as memfds marked as DMA-BUFs, so that tests can
follow the whole path with lavapipe, and `WPRS_DMABUF=emulate-flaky`, where every second
allocation fails.

## `io.github.wallpaper_rs.Daemon1`

### `RequestStream(s output, u width, u height) → (u node_id, t serial)`

Adds a claim of the caller on `output` and creates or reuses the PipeWire video stream at
`width` × `height` physical pixels. Returns the node id and `object.serial` of the stream node.
Consumers should connect by serial; `serial` is `0` only if PipeWire did not publish one within
2 s, in which case the node id can be used.

- `output`: connector name such as `DP-1`.
- A request with the same size returns the existing stream.
- A request with a different size replaces the stream; the old serial becomes invalid.
- Each edge must be in `1..=min(16384, GPU limit)` and one frame, with rows padded to 256
  bytes, must fit in 256 MiB and in the GPU buffer limit (for example 8192 × 8192); otherwise
  `InvalidArgs`.
- A stream is never larger than the screen: when the daemon knows the pixels of the output (the
  current mode of the `wl_output` with that name, turned with the output, while the daemon is
  connected to the Wayland compositor, which it reconnects to when the compositor restarts;
  otherwise `width` × `scale` from `SetOutputInfo`, which only reduces a request that was not
  built from that same scale), a larger request is reduced to them keeping its aspect ratio, and
  `GetState` shows the size used. When the pixels of the screen change, the stream follows them
  again, up to the size requested. A later request of the same size returns that stream.
- Call `ReleaseStream` once for every successful `RequestStream`.

Stream format: raw video, `BGRx` (or `BGRA`), fixed size, variable frame rate, buffers in GPU
memory (DMA-BUF) or one memfd per buffer mapped from offset 0 (see "Stream buffers"). Frames are
produced when the picture changes (new wallpaper, transition, video frame, a scene that animates or
whose inputs changed), when a consumer starts streaming or another consumer links to the stream,
again 50 ms, 250 ms, 1.25 s and 6.25 s later (so a consumer that was not ready for the first frame,
or a link that was still being set up, gets the last picture), and every 10 s as a keep-alive.
While no consumer is streaming an output, its video is paused and the daemon renders only the final
picture after each change, which a consumer receives as soon as it connects.

### `HoldStream(s output, u width, u height) → (u node_id, t serial)`

Same as `RequestStream`, but with the held claim. Used by `wprs-ctl stream` for tests and
tools that exit right after the call.

### `ReleaseStream(s output)`

Removes one claim of the caller on `output`, or the held claim if the caller has none.

### `SetOutputCovered(s output, b covered)`

Reported by the desktop integration when a maximized or fullscreen window covers `output`.
See "Streams and claims".

### `SetOutputInfo(s output, i x, i y, u width, u height, d scale, d px_per_mm, s identity)`

Tells the daemon where the screen of `output` sits, for the `span` fit and the per-screen crops of
edits (see "Edits and spanning"). `x`, `y`, `width` and `height` are its logical position and
size in the desktop layout, `scale` its device pixel ratio (`0.1` to `16`), `px_per_mm` its
density in physical pixels per millimetre (`0` when unknown) and `identity` the maker, model and
serial number of the monitor (at most 256 bytes, no control characters). The desktop
integration calls it after every successful `RequestStream`; layer-shell outputs take the same
values from `wl_output` and `xdg_output`, and X11 outputs from RandR. `NotFound` when `output`
has no stream. When the screen has fewer pixels than the stream, or more pixels than a stream
that was reduced, the daemon creates the stream again at the size of the screen (never above the
size requested) and emits `StreamChanged`.

### `PreviewEdit(s path, s edit_json)`

Shows the image `path` with `edit_json`, an edit as described in "Edits and spanning", instead
of its saved edit wherever `path` is shown, without saving anything. Calls replace the preview
and are rendered at most once per frame; an empty `edit_json` ends it, and so do 60 s without a
call. `path` is absolute; a relative path, an invalid edit (the message names the field) or more
than 2 MiB of JSON return `InvalidArgs`, a missing file `NotFound`.

### `PreviewScene(s output, s dir, s document, d time)`

Shows a scene that is being edited on `output`, or on every output when `output` is empty,
instead of what the output shows, without saving or applying anything (see `editor.md`).
`dir` is the absolute path of the scene folder, whose files provide the assets, and `document`
is `{"manifest": …, "scene": …}`, the unsaved manifest and `scene.json`. A `time` of 0 or more
holds the scene at that second, as a still frame that only renders again when the call changes;
a negative `time` plays it. The scene is checked for what it needs to render: the id, the user
properties, the presets and the scene itself, while texts, rating, origin and credits may still be
incomplete; an invalid document, a path outside `dir` or an asset over the limits return
`InvalidArgs`. Calls replace the preview of the
output and reuse the decoded assets of the previous one, without a crossfade; a playing preview
of the same folder keeps its clock, so an edit does not restart it. An empty `document` ends it,
and so do 60 s without a call or the output going away. Outputs whose name starts with
`preview:` are virtual: a client creates one with `RequestStream` to show the preview inside its
own window, and it is left out of `GetState`, of the conditions and holds of profiles, of
playlists and of the record of what was shown, so it never counts as a screen.

### `SetWallpaper(s path, s output, s fit)`

Sets the wallpaper of `output`, or of every output when `output` is empty (this also clears
per-output wallpapers). `path` is an image or a video (`mp4`, `m4v`, `webm`, `mkv`, `mov`,
`avi`, `ogv`, `mpg`, `mpeg`); videos loop and are decoded with GStreamer. `fit` is `fill`,
`fit`, `stretch`, `center`, `tile`, `span`, or empty to keep the current mode. Only local files are
accepted: an absolute path or a `file://` URI with an empty or `localhost` host, which the
daemon percent-decodes; any other URI returns `InvalidArgs`. Clients should still convert URIs
to paths themselves, as the other methods reject URIs. A `.wpak` package returns `InvalidArgs`
(`import the package and use Library1.Apply`). Images are decoded before the configuration is
saved. A new wallpaper replaces the old one with the configured transition.

A non-empty `path` also stops the playlist of `output` (of every output when `output` is
empty) and clears the `playlist` and `preset` of the affected configuration sections. If `path`
is a library item, `GetState().outputs[x].item` reports its uid. No history is recorded.

With an empty `path`, only the fit changes and the current wallpapers stay: the fit of
`output`, or of every output when `output` is empty. A fit for every output also clears
per-output fits, with or without a path. An empty `path` and an empty `fit` return
`InvalidArgs`.

### `SetBackground(s color, s output)`

Sets the color shown around and behind the wallpaper of `output`, or of every output when
`output` is empty (this also clears per-output colors). `color` is `#rrggbb`; anything else
returns `InvalidArgs`.

### `GetSceneProperties(s output) → (s json)`

A `SceneProperties` document for the package shown on `output`, or for `output = ""` the one set in
the `[default]` section: a scene package or a video package (`kind`), its settings as declared in
its manifest (for a video package, the playback settings described in `package.md`), their current
values, its presets and the presets the user saved for it. `NotFound` when no such package is shown
there. `fields` lists the settings in the order to show them (by `order`, each group followed by its
members), with labels, help and option labels in the language of the daemon's locale, `visible`
false while a `visible_if` condition of the setting or of its group does not hold and `enabled`
false while an `enabled_if` condition of the setting or of its group does not hold; `preset_labels`
names the presets of the package the same way and `user_presets` the saved ones (`value` is their
uid). Clients draw their forms from `fields` alone: `decimals` (always given for sliders, 0 for
whole numbers), `step`, `editable` and `log` describe sliders, a `divider` without a label has
`label` `""`, `icon` is one of the icons of `scene.md` or `""`, `group` names the group of a member,
`collapsed` is the initial state of a group, and `accept` the kinds of file a `file` or `folder`
takes. The value of a `file` or `folder` field is the chosen path or `null` (the package's own
asset). A `secret` text never shows its value: `filled` tells whether one is stored. `shared` is
true when the screen follows the values of the `[default]` section rather than its own. `warnings`
lists stored values that the package no longer accepts (a setting removed, retyped or with a
narrower range after an update), which are ignored for their defaults (`scene.md`).

### `SetSceneProperty(s output, s id, s value_json)`

Sets one setting of the package shown on `output` (`""` = the `[default]` section, used by every
output without its own values) to `value_json`, a JSON value of the setting's type: a `#rrggbb` (or
`#rrggbbaa` with `alpha`) string for `color`, a number for `slider`, `true` or `false` for `bool`,
the value of one of the options for `combo`, a string for `text`, and an absolute path or `null` for
`file` and `folder`. Numbers out of range are clamped, aligned to the step and rounded to the
decimals; anything else that does not fit, an unknown `id`, a `button`, `group`, `label` or
`divider`, a file of a kind the setting does not accept or that cannot be read as an image or a
video, and a folder that is not one return `InvalidArgs`. Values are stored per package id in the
configuration file (see "Configuration file"), never inside the package: in the section of `output`
when that screen has its own values for the package, otherwise in the `[default]` section that the
screens without their own values share. They apply at once without reloading the scene. A `secret`
text is stored in the Secret Service instead, once for every output (`KeyringUnavailable` without
one), and `""` forgets it; it never reaches the configuration, the logs, exported settings or sync.

### `ApplyScenePreset(s output, s preset)`

Stores every value of a preset as `SetSceneProperty` would: a preset of the package by its id or its
label, or a saved preset by its uid or name, ignoring case. Files, folders and secrets are never
part of a preset. `NotFound` for a preset that does not exist.

### `ResetSceneProperties(s output)`

Forgets the values stored for the package shown on `output` (`""` = the `[default]` section): the
screen's own values when it has them, which it keeps having (empty), or else the shared ones, so its
settings take their defaults. Secrets stay stored.

### `SaveScenePreset(s output, s name) → (s preset)`

Saves the current values of the package shown on `output` as a preset named `name` and returns its
uid; saving a name the package already has replaces that preset and keeps its uid. Saved presets
belong to the package id, so they survive updates of the package, live in the library and travel
with sync to the user's other devices.

### `RenameScenePreset(s preset, s name)`

Renames a saved preset. `Exists` when the package already has a preset of that name.

### `DeleteScenePreset(s preset)`

Deletes a saved preset.

### `ExportSceneValues(s output, s preset) → (s json)`

The current values of the package shown on `output`, or those of a preset when `preset` is not
`""`, as a small document `{"format": "wpak-values/1", "package", "version", "values"}` that the
user may keep, copy or share. Files, folders and secrets are left out.

### `ImportSceneValues(s output, s json, s name) → (as warnings)`

Reads a `wpak-values/1` document of the package shown on `output` (at most 256 KiB): with
`name = ""` its values are stored as `SetSceneProperty` would, otherwise they are saved as a preset
named `name`. A document of another package returns `InvalidArgs`; settings that the package does
not have or no longer accepts, files, folders and secrets are skipped and listed in `warnings`.

### `PressSceneButton(s output, s id)`

Presses the `button` setting `id` of the scene shown on `output` (`""` = every output that shows
it): timelines whose `trigger` is `prop:<id>` start again and the scripts of the scene see the press
in their next frame (`scene.md`). Nothing is stored.

### `ShareSceneValues(s output, b shared)`

With `shared`, the screen `output` stops having its own values for the package it shows: they move
to the `[default]` section, which every screen without its own values follows. Without it, the
screen gets its own copy of the shared values, which later changes on it keep for itself.
`InvalidArgs` for `output = ""`.

### `SetOption(s key, s value)`

Changes one setting and saves the configuration. Keys and their current values are listed in
`GetState().options`, and described in `GetSchema().options`:

| Key | Values | Default |
|-----|--------|---------|
| `playback.volume` | `0.0` to `1.0`, for videos and scenes with sound | `0.5` |
| `playback.muted` | `true`, `false`, for videos and scenes with sound | `true` |
| `playback.max_fps` | `1` to `240`, or `display` (also `0`) for the refresh rate of each output on mains power and 30 on battery (see "Frame rate") | `display` |
| `playback.software_parallax` | move depth parallax images even when the GPU is emulated on the CPU (such as llvmpipe), with 8 steps at 15 frames per second at most | `false` |
| `transition.kind` | `fade`, `none` | `fade` |
| `transition.duration_ms` | `0` to `10000` | `600` |
| `pause.when_covered` | pause while a maximized or fullscreen window covers the output | `true` |
| `pause.on_battery` | pause while running on battery | `true` |
| `pause.when_locked` | pause while the screen is locked; whatever it says, videos are muted and scenes play no sound while it is locked | `true` |
| `library.watch` | rescan folder sources when files in them change, one scan per source at most every 5 s | `true` |
| `library.thumb_cache_mb` | `64` to `8192`, size of the thumbnail cache | `512` |
| `library.hide_mature` | leave packages rated `mature` out of `Search` (unless the query has its own `content:` filter; `SearchPage.query` stays the one given) and `Browse`; a change emits `library` and `repos` | `false` |
| `repos.auto_refresh` | refresh each repository 10 min after start and then every 24 h | `true` |
| `layout.bezel_mm` | `0` to `100`, frame width of every screen side for the `span` fit | `0.0` |
| `models.allow_download` | let `wprs-ctl models download` and the app download the depth model and ONNX Runtime after asking | `true` |

Only the changed key is validated. Unknown keys and invalid or out-of-range values return
`InvalidArgs` and change nothing.

### `Reload()`

Re-reads the configuration file and applies it. If the file is invalid, the current
configuration stays, `config_error` is set and the call returns `Failed`.

### `GetState() → (s json)`

A `State` document (see "JSON model"). It never waits for the library: the library item, preset,
playlist and credit of each output are kept up to date when they change.

### `GetSchema() → (s json)`

A `Schema` document describing every value a client would otherwise hard-code: the keys of
`SetOption`, `Playlists1.SetRule`, `Profiles1.SetAuto` and `Profiles1.SavePreset`, the provider
kinds and their options, the search keys, the thumbnail sizes and the fits. It carries no
labels: clients translate keys with their own strings and fall back to the key. An `int` field
lists in `values` the words it also takes, such as `display` for `playback.max_fps`. It never
changes while the daemon runs.

### `SetPaused(b paused)`

Pauses or resumes every animated wallpaper.

### `SetVolume(d volume)`

Sets the volume of video wallpapers and scenes with sound, `0.0` to `1.0`. Their sound goes to
PipeWire directly (`pipewiresink`, stream `wallpaper-rs` with `media.role = Movie`) when the
GStreamer PipeWire plugin 1.2 or newer is installed; without it, with an older one (whose buffer
pool can deadlock), or when PipeWire cannot be reached outside the Flatpak, it goes to the default
audio output of GStreamer. While PipeWire has no audio output (and in the Flatpak while PipeWire
cannot be reached) the video plays without sound and its sound goes back to PipeWire, at the same
position, once an audio output appears. Videos with sound loop by seeking back at their end.

### `SetMuted(b muted)`

Mutes or unmutes video wallpapers and scenes with sound (see "Scene sound"). While the screen is
locked they stay silent whatever this says.

### `WaitChanged(t serial, u timeout_ms) → (t serial, as topics)`

Long poll for scripts; see "Change notification". `timeout_ms` is clamped to `0..=20000`.

### `ListJobs() → (s json)`

A `JobsReply`: queued and running jobs plus the jobs that finished in the last 10 minutes (at
most 64 of them).

### `GetJob(t job) → (s json)`

A `JobReply`. `NotFound` for a job that is unknown or finished too long ago.

### `CancelJob(t job)`

Cancels a job; see "Jobs". `NotFound` for an id this daemon run never issued.

### Property `Version` (`s`)

Daemon version.

### Property `Adapter` (`s`)

GPU adapter and backend in use.

### Property `Paused` (`b`)

Paused by the user. `PropertiesChanged` is emitted whenever it changes.

### Property `Volume` (`d`)

Volume of video wallpapers and scenes. `PropertiesChanged` is emitted whenever it changes, whichever
method changed it (including `SetOption` and `Reload`).

### Property `Muted` (`b`)

Video wallpapers and scenes are muted. `PropertiesChanged` is emitted whenever it changes. It
does not follow the screen lock, which silences them too.

### Property `Serial` (`t`)

Change counter; see "Change notification". No `PropertiesChanged` is emitted for it.

### Property `Model` (`u`)

Version of the JSON data model of the replies; constant.

### Signal `StreamChanged(s output, u node_id, t serial)`

Emitted for every output with a stream after the daemon reconnects to a restarted PipeWire, as
soon as the stream of `output` exists again under a new node. The claims stay valid, but
consumers lose their own PipeWire connection and must connect again to `serial` (or `node_id`
when `serial` is `0`).

### Signal `Changed(s topic, as ids, t serial)`

Something of `topic` changed; `serial` is the new value of `Serial`. See "Change notification".

### Signal `JobProgress(t job, t done, t total)`

Progress of a running job, at most 4 times per second per job; `total = 0` means unknown.

### Signal `JobFinished(t job, s state, s error_name, s message)`

A job ended. `state` is `done`, `failed` or `cancelled`; `error_name` and `message` are empty
unless the job failed. A cancelled job has `message = "cancelled"`.

## `io.github.wallpaper_rs.Library1`

The library catalogs images, videos and `.wpak` packages found in folder sources or imported
one by one. Every method returns `Failed` while the library is unavailable.

### `AddSource(s path, b recursive) → (s source, t job)`

Adds a folder and starts a `scan` job of it. `path` is canonicalized. `InvalidArgs` when it is
not an existing directory, when it would share files with an existing source (the same folder, a
folder inside a recursive source, or a recursive folder containing a source: `overlaps the
source <path>`), and when it lies inside `~/.cache/wallpaper-rs` or
`~/.local/share/wallpaper-rs` or, being recursive, contains one of them through non-hidden
folders (`contains wallpaper-rs data`). Scans skip hidden entries, so adding the home folder is
allowed, and depth maps (`*.depth.png`), which are never items: `Import` refuses them and a scan
marks one already in the library missing.

### `RemoveSource(s source)`

Cancels a queued or running scan of the source, then removes it with its items (their tags,
ratings, playlist entries and thumbnails). Sources that belong to a provider return
`InvalidArgs` (`belongs to provider "<id>"; remove the provider`).

### `ListSources() → (s json)`

A `SourcesReply`: every source with its item count and, for provider cache folders, the
provider id.

### `Rescan(s source) → (t job)`

Starts a `scan` job. `source = ""` scans every source in one job and then checks the items that
belong to no source (imported files and packages), so that deleted files become `missing`.

### `Import(as paths, b copy) → (t job)`

Imports files one by one in an `import` job. Each path must be absolute (no URI); directories go
through `AddSource`. With `copy = false` the file is cataloged where it is. With `copy = true`
it is first copied to `~/.local/share/wallpaper-rs/files/` (on a name collision as
`<stem> (2).<ext>`, `(3)`, …) and takes the original file name without extension as title. A
file that fails is listed in `ImportReport.failed`; the job fails only on a global error.

### `Resolve(as refs) → (as uids)`

Turns references into uids, in order. A reference is an exact uid (any case), a unique uid
suffix of at least 6 characters (any case), or the absolute path of a known item. The first
failing reference returns `NotFound` (`no item matches "<ref>"`) or `InvalidArgs`
(`"<ref>" matches N items; use more characters`, or a suffix shorter than 6 characters).

### `Remove(as uids, b delete_files)`

Removes items with their thumbnails and materialized package entries, and with `delete_files`
their files. Every uid is checked first (`NotFound` names the first unknown one); nothing
changes and `InvalidArgs` is returned when an item is a package installed from a repository
(`use Repos1.Uninstall`), belongs to a source while its file still exists and is no depth map
(`remove the file or the folder`: the next scan would add it back), or lies outside
`~/.local/share/wallpaper-rs/files/` while `delete_files` is true. `ItemSummary.removable` and
`imported` tell clients in advance.

### `Search(s query, u offset, u limit) → (s json)`

A `SearchPage`. Free text matches the start of words in titles, tags and paths; `key:value`
filters (listed in `GetSchema().query`), `-` negates and quotes group words. `sort:` orders the
results (newest first by default). `limit` is `0..=500`; `0` returns only `total`.

The query is normalized first and `SearchPage.query` echoes it: `in:<name>` becomes `in:<uid>`
(`in:` takes a manual playlist; a smart or unknown playlist returns `InvalidArgs`), and a bare
`sort:random` becomes `sort:random:<seed>` with a new seed, which orders the same way every time.
Clients pass the normalized query unchanged for later pages, so random pages neither repeat nor
skip items. `ItemSummary.thumb` is the cached 256 px thumbnail or `null`: `Search` never makes
thumbnails. `ItemSummary.package` describes package items, whose `kind` tells scenes apart.

### `GetItem(s uid) → (s json)`

An `ItemDetails`: the item with its path, the cached 512 px thumbnail, its presets, the manual
playlists that contain it, and its provider credit, installed package record and revocation
reason when they apply. For a valid package, `about` holds its manifest metadata (`package.md`)
with the texts in the user's language: `description` and `changes` keep their markup, which
clients render without loading anything remote, opening a link only after a click and a
confirmation, except the description of a first-version package (`schema` is
`wallpaper-rs.wpak/1`), which is plain text shown as written. Every other text is plain.
`provider` is the credit of the online source it comes from, `platforms` the devices it is made
for, `packages` the packages it needs (`<id> <version requirement>`) and a credit's `source` the
package its files come from (`<id> <version>`). A package this device cannot play keeps its
details, with the reason in `error` (`this package needs a newer version of wallpaper-rs: it uses
…` or `this package is made for … only`).

### `SetTitle(s uid, s title)`

Renames an item.

### `SetRating(as uids, u rating)`

Sets the rating, `0` to `5`, of every item.

### `SetFavorite(as uids, b favorite)`

Sets or clears the favorite flag of every item.

### `SetTags(as uids, as add, as remove)`

For every item, adds the tags of `add` and then removes those of `remove`, in one transaction.
Tag names compare without case; the first spelling stored is kept.

### `ListTags() → (s json)`

A `TagsReply`: every tag in use with its item count.

### `RenameTag(s from, s to)`

Renames a tag; if `to` exists (without regard to case) the two merge. Renaming to the same name
with another case changes the spelling. `NotFound` for an unknown `from`.

### `Thumbnail(s uid, u size) → (s path)`

Returns the path of the item's thumbnail of `size` (`256`, `512` or `1024`, else
`InvalidArgs`), making it first when it is not cached, ahead of every other thumbnail work.
`Failed` with the reason when it cannot be made or takes more than 20 s; a decoding problem also
sets the item's `error`. A scene package without preview returns `Failed` (`no preview`). A
package that needs a newer wallpaper-rs keeps that `error` and still gets the thumbnail of its
preview. The library reads every package again once after the daemon is upgraded.

### `RequestThumbnails(as uids, u size) → (a{ss} paths)`

Returns `{uid: path}` for every item whose thumbnail of `size` is cached, and queues the others
ahead of background analysis; `ThumbnailReady` follows for each of them (`path = ""` when it
cannot be made or the file is missing). A new call from the same sender replaces that sender's
queued requests, so a fast scroll does not queue the whole library. Unknown uids are ignored.

### `Apply(s uid, s output, s fit, s preset)`

Shows the item on `output` (`""` = every output, clearing per-output wallpapers and
playlists) and stops the playlists of the affected outputs. `fit` is a fit or `""` to keep the
current one. `preset` is `""`, a preset uid, or a preset name looked up first among the item's
presets and then among the global ones; its `fit` and `background` are applied first, and an
explicit `fit` wins over the preset's. `Failed` before changing anything when the item is
missing, has an `error` (invalid packages), or is a revoked package, and `Unconfirmed` when it
is a photosensitive package the user has not confirmed: clients warn that it flashes or shows
fast patterns that can trigger seizures, and apply it only after the user agrees. The item's path,
the effective fit and background and the preset uid (as a label) are written to the affected
configuration sections, and the use is recorded once per affected connected output. Image and
video packages show their entry: it is copied once to `~/.cache/wallpaper-rs/blobs/`. Scene
packages are rendered live (see "Scenes"); the videos they contain are copied there once.

### `Confirm(as uids, s what)`

Records a decision of the user about the packages of these items, once per package id, so that
every version and copy of the package keeps it. `what` is `mature` (clients stop blurring the
thumbnails of a package rated `mature`) or `photosensitive` (`Apply` and playlists may show it);
anything else returns `InvalidArgs`, an unknown uid `NotFound`. Items that are no packages are
accepted and change nothing. `PackageInfo.confirmed` lists the decisions, and the `library` change
names every item that shares a package id with these items.

### Signal `ThumbnailReady(s uid, u size, s path)`

A thumbnail queued by `RequestThumbnails` is ready, or could not be made (`path = ""`).

## `io.github.wallpaper_rs.Playlists1`

Playlist arguments take a full uid or a name (names compare without case). Rotation runs in the
daemon and needs no client:

- A playlist switches at every `interval` and at every `at` time (local time), on the listed
  `days` only. A time skipped by a daylight saving jump fires right after the gap; a time that
  occurs twice fires once. `Next` and `Previous` work on any day and restart the interval.
- The daemon sleeps until the nearest switch, waking at least every 15 minutes to follow the wall
  clock, and recomputes after a resume from suspend and when the configuration, the power
  source, the pause state, the outputs or the library change. With no switch pending (no
  playlist shown, paused outputs, empty playlists, no timer) it does not wake at all.
- When the wall clock is set or the time zone changes (`org.freedesktop.timedate1` reports a new
  `Timezone`; in the Flatpak, whose `/etc/localtime` keeps the zone the sandbox started with, the
  daemon follows `/run/host/monitor/timezone` instead), the next `at` time is computed again at
  once; an interval keeps its next switch unless the clock moved back past it, and an overdue
  switch happens once.
- While a `scan`, `import`, `analyze` or `provider_sync` job runs, library changes resolve the
  playlists shown at most once every 3 seconds.
- Each output has its own position (`synced = false`), or every output showing the playlist
  shares one (`synced = true`) and shows the same item. Positions are kept in the library, so a
  restart shows the same item (`resume = true`) or the next one (`resume = false`).
- Adding, removing or reordering items, or a scan, never changes the item shown; if that item
  is gone, the item at its position is shown. Shuffled playlists never repeat an item within a
  cycle.
- While an output is not active (paused, covered, locked or paused on battery) its playlist does
  not switch unless `advance_while_paused` is set; when it becomes active and a switch is
  overdue, it switches once. Disconnected outputs do not switch either, and switch once when
  they come back after a missed switch.
- Missing items, items that cannot be decoded, revoked packages and photosensitive packages the
  user has not confirmed are left out, and so are videos on battery with
  `skip_videos_on_battery`. If a playlist has nothing to show, the output
  keeps its last picture, or shows its background color with the reason in
  `GetState().outputs[x].error`.
- `GetState().outputs[x]` reports the `item` shown and `playlist` (`uid`, `name`, `position` in
  the cycle, `count` of playable items, `next_at`). Every shown item is recorded in `History`.

### `List() → (s json)`

A `PlaylistsReply`.

### `Get(s playlist, u offset, u limit) → (s json)`

A `PlaylistPage`: the playlist and a page of its items. Manual playlists list their entries in
stored order (missing items included); smart playlists list the current result of their query.
The item at `items[i]` is entry `offset + i`. `limit` is `0..=500`.

### `Create(s name, s query) → (s uid)`

Creates a manual playlist (`query = ""`) or a smart one. The query is validated and normalized
as in `Library1.Search`. `Exists` for a name in use.

### `Rename(s playlist, s name)`

Changes the name only.

### `SetQuery(s playlist, s query)`

`""` turns a smart playlist into a manual one; a query turns a manual playlist into a smart one.
The manual entries stay stored and come back when the query is cleared.

### `Delete(s playlist)`

Stops the playlist on its outputs (the picture shown stays), then deletes it with its positions.

### `AddItems(s playlist, as uids)`

Appends items to a manual playlist; an item may appear more than once. `InvalidArgs` for a smart
playlist.

### `RemoveItems(s playlist, as uids)`

Removes every occurrence of each item from a manual playlist.

### `RemoveEntries(s playlist, au indexes, as expected)`

Removes the entries at `indexes` when `expected[i]` is the uid stored at `indexes[i]`;
otherwise nothing changes and `Conflict` is returned.

### `MoveItem(s playlist, u from, u to, s expected)`

Moves the entry at `from` to `to` when the entry at `from` is `expected`; otherwise `Conflict`.

### `SetRule(s playlist, s key, s value)`

Sets one rotation rule; the keys are listed in `GetSchema().rules`. Changing `synced` keeps the
item shown as the start of the new positions; any change restarts the interval.

| Key | Value | Default |
|-----|-------|---------|
| `order` | `sequential`, `shuffle` | `sequential` |
| `interval` | `0s` (no timer) or `10s` to `30d`, such as `90s`, `1h30m`, `1d` | `30m` |
| `at` | comma-separated `HH:MM` local times, at most 24; `""` clears | `""` |
| `days` | comma-separated weekdays `Mon` to `Sun`; `""` = every day | `""` |
| `synced` | `true`, `false`: one cursor for every output showing the playlist | `false` |
| `resume` | `true`, `false`: show the current item again after a restart | `true` |
| `advance_while_paused` | `true`, `false` | `false` |
| `skip_videos_on_battery` | `true`, `false` | `false` |
| `transition_ms` | `0` to `10000`; `""` = `transition.duration_ms` | `""` |

### `Start(s playlist, s output)`

Assigns the playlist to `output` (`""` = every output, clearing the wallpapers and playlists of
single outputs), writes it as `playlist` of that configuration section and clears its `image`
and `preset`, then shows the current or first item at once (the next one with `resume = false`)
and restarts the interval. `Failed` (`the playlist has no items to show`) when nothing is
playable; nothing changes then.

### `Stop(s output)`

Removes the playlist of `output` (`""` = every output that has one) and keeps the item shown as
the fixed wallpaper (written as `image`). An output without playlist is left as it is.

### `Next(s output)`

Shows the next item on `output` (`""` = every output with a playlist) and restarts the
interval. A synced playlist moves on every output that shows it. `NotFound` for an output
without playlist.

### `Previous(s output)`

Goes one step back on `output`, like `Next`: from the first item of a shuffle cycle to the last
item of the previous cycle; at the very start nothing changes.

### `History(s output, u limit) → (s json)`

A `HistoryReply`, newest first. `output` is a connector name; `limit` is `1..=1000`.

## `io.github.wallpaper_rs.Profiles1`

Profiles store a complete assignment of wallpapers to outputs, optionally with settings;
presets store a fit and a background color, for one item or for every item. Profile arguments
take a full uid or a name (names compare without case).

A profile with rules (`SetAuto`) is activated automatically when every rule it sets holds: after
the set of connected outputs has stayed the same for 2 seconds following a connect or
disconnect, and when the power source changes. The highest `priority` wins, ties go to the
first name. A manual `Activate` holds until the next such change; when nothing matches, nothing
changes. At start the rules wait for the power source (at most 5 s), so a battery state not yet
known does not end a hold. The choice also survives a restart of the daemon: the profile chosen
with `Activate` (or cleared with `""`) and the connected outputs and power source at that moment
are kept in `~/.local/share/wallpaper-rs/profile-hold.json` (`0600`) until the outputs or the
power source differ from them. A moment without any output, as while the daemon starts, does not
end the hold.

### `List() → (s json)`

A `ProfilesReply`.

### `Get(s profile) → (s json)`

A `ProfileReply`.

### `Save(s name, b with_options) → (s uid)`

Saves the current assignment of every output, and with `with_options` every setting except
`models.allow_download` (which `Activate` never applies either), as a profile. An existing
profile of that name is replaced and keeps its uid, rules and priority.

### `Activate(s profile)`

Applies a profile at once and marks it active; `""` only clears the active marker. Outputs not in
the profile lose their own wallpaper. Entries whose item, file or playlist is gone or cannot be
shown are skipped with a warning, and those outputs show the default section; so are photosensitive
packages the user has not allowed (`Confirm`), and `GetState().outputs[x].error` says why until that
output is changed again. Unknown or invalid settings are skipped too. The configuration is saved
once and every output is reconfigured.

### `Rename(s profile, s name)`

Renames a profile.

### `Delete(s profile)`

Deletes a profile; deleting the active profile clears the marker.

### `SetAuto(s profile, s key, s value)`

Sets a rule for automatic activation: `outputs` (comma-separated connector names, met when
exactly these outputs are connected; `""` unsets), `on_battery` (`true`, `false`; `""` unsets)
or `priority` (`-1000` to `1000`).

### `ListPresets(s item) → (s json)`

A `PresetsReply`: the presets of `item`, or the global presets for `item = ""`. Presets of the
settings of a package (`SaveScenePreset`) are listed by `GetSceneProperties` instead.

### `SavePreset(s item, s name, a{ss} values) → (s uid)`

Saves a preset of `item` (`""` = global) with `fit` and/or `background` (`#rrggbb`). An
existing name in that scope is replaced and keeps its uid, and the configuration sections that
use it get its new fit and background.

### `DeletePreset(s preset)`

Deletes a preset; configuration sections that used it keep their fit and background.

## `io.github.wallpaper_rs.Providers1`

Providers download wallpapers from online services into a cache folder,
`~/.cache/wallpaper-rs/providers/<id>/` (`0700`), that is a library source; playlists use their
items through the library (for example `Playlists1.Create(name, "source:<source uid>")`). Kinds
are `bing`, `apod`, `wikimedia`, `wallhaven`, `webdav`, `nextcloud` and `immich`; their options,
defaults, contacted hosts and secrets are listed in `GetSchema().providers`. Local folders are
`Library1` sources, not providers. Provider ids match `^[a-z][a-z0-9_-]{0,31}$`.

Secrets (passwords and API keys) are written only by `Add` and `SetSecret`, to the Secret
Service (`org.freedesktop.secrets`, default collection, falling back to `login`) as an item
labelled `wallpaper-rs: <id>` with the attributes `xdg:schema = io.github.wallpaper_rs.Provider`
and `provider = <id>`. They are never stored anywhere else and never appear in replies, signals,
job errors, logs or `config.toml`. A secret has at most 4096 bytes and no control characters.
Only `Add` with a secret, `SetSecret`, `ForgetSecret`, `Set` of `url`, `server` or `user`,
`Remove` and an explicit `Sync` may show the keyring's unlock prompt; they wait at most 20 s and
then return `KeyringUnavailable` (`unlock the keyring and try again`) without storing or
deleting anything. Clients call `Add` and `SetSecret` with a timeout of 120 s. A Secret Service
that does not answer gives `KeyringUnavailable` (`the keyring did not answer in time`) after
10 s for lookups (`List` then reports `has_secret = false`, and scheduled syncs wait for their
next due time) and after 100 s for writes, deletions and reads that may prompt.

URL options (`url`, `server`) must be `https://`, or `http://` to this computer; any other
`http://` URL needs the option `allow_plain_http = true`, given explicitly. URLs with user
information, a query or a fragment return `InvalidArgs` (`put credentials in the secret, not in
the URL`). Requests that carry a secret never follow a redirect to another origin or from HTTPS
to HTTP.

A sync job lists the provider's items and downloads them in list order into the cache folder.
Files of items shown on an output, favorite, rated above 0 or in a manual playlist are protected:
downloading stops when the other files would exceed `cache_mb` minus the protected files (the
rest count as `skipped`), and older unprotected files are then evicted. The folder is scanned
(evicted items become `missing` and never rotate), the items get the provider's titles while
their title is still the file name, and their credits (title, attribution, license, and a link
for public services) are kept in the folder's `.credits.json` and reported as
`ItemDetails.credit` and `GetState().outputs[x].credit`.

A provider whose `refresh` is not `0s` syncs automatically when it has never synced or its last
sync is older than `refresh`; the schedule is checked again 5 s after `Add`, `Set` and
`SetSecret`. Failed syncs are retried after 30 s, doubling up to 1 h, and cancelled ones after
1 h. A provider whose required secret is missing is in state `needs_secret` and does not sync
automatically; one whose secret was rejected (HTTP 401 or 403, `NeedsSecret`) waits for
`SetSecret`. Automatic syncs never show an unlock prompt: with a locked keyring the provider's
state becomes `locked` and the sync waits for its next due time.

### `List() → (s json)`

A `ProvidersReply`. Entries of `config.toml` that cannot be read (an unknown kind or option, a
bad value, a secret written by hand) are listed with state `error`, their reason in
`last_error` and no options; they are never synced and are kept in the file unchanged.

### `Add(s id, s kind, a{ss} options, s secret) → (s source)`

Validates everything, then stores a non-empty `secret` in the keyring (`KeyringUnavailable`:
nothing is created), registers the cache folder as a library source, writes
`[providers.<id>]` to `config.toml`, and returns the source uid. `secret = ""` stores none; a
kind whose secret is required then starts in `needs_secret`. On a kind with a secret,
`secret = ""` also makes sure that no secret left in the keyring under the same id (by a
provider removed while the keyring could not be reached) reaches the new provider: such an item
is deleted when that needs no unlock prompt; when the keyring is locked, `Add` returns
`NeedsSecret` and creates nothing, and the message asks to give the secret (which replaces the
item) or to unlock the keyring and add the provider again. `InvalidArgs` for an invalid id,
an unknown kind, option or value, a missing required option, an option named like a secret
(`password`, `api_key`, `token`, `secret`), a refused URL, or a secret given to a kind without
one; `Exists` for a taken id.

### `Set(s id, s key, s value)`

Changes one option with the same checks. Cached files stay until the next sync replaces them.
On a kind with a secret, changing `url`, `server` or `user` also deletes the stored secret first
(`KeyringUnavailable`: nothing changes), so a new host never receives the old secret.

### `Remove(s id, b purge)`

Cancels a running sync, deletes the stored secret (`KeyringUnavailable` only when it exists and
cannot be deleted; then nothing is removed), the library source with its items and thumbnails,
and the configuration entry. `purge` also deletes the cache folder.

### `Sync(s id) → (t job)`

Starts a `provider_sync` job; `id = ""` syncs every provider that is not `needs_secret`. A
provider that another sync job is already syncing is skipped. `NeedsSecret` when the provider's
required secret is missing.

### `SetSecret(s id, s secret)`

Stores the provider's secret, replacing the previous one. `InvalidArgs` for an empty secret or
a kind without secret.

### `ForgetSecret(s id)`

Deletes the provider's secret. Kinds without secret never touch the keyring.

## `io.github.wallpaper_rs.Repos1`

Signed repositories of `.wpak` packages (see `repo.md`), kept in `~/.local/share/wallpaper-rs`.
`repo` arguments take a full repository id or a unique prefix of at least 8 hex digits. Trust is
anchored by the URL a repository is added with (`fingerprint=` and/or `repo_id=`); repository
jobs (`repo_add`, `repo_refresh` and the download of `install`) run one at a time. Requests to
a repository follow redirects only to its declared bases (the URL it was added with and the
mirrors of its signed root), never from HTTPS to HTTP. Installed packages are imported into the
library; revoked ones are never applied again (`Apply` returns `Failed`), rotation skips them,
and an output showing one when its revocation arrives shows its background color instead.

### `List() → (s json)`

A `ReposReply`. `stale` and `warnings` are computed at every call; `last_error` is the message
of the last failed refresh since the daemon started.

### `Add(s url) → (t job)`

Adds a repository whose URL carries its trust anchor. The URL is `https://`, `file://` or
`http://` to this computer, without user information or fragment. `Exists` at once when its
`repo_id=` names an added repository, or when the job finds it already added. The job verifies
the repository fully and stores its signed state.

### `Refresh(s repo) → (t job)`

Refreshes a repository (`""` = all) and applies revocations to the installed packages, also
when the repository is stale. A failed refresh keeps the last verified state. With
`repos.auto_refresh`, every repository is refreshed 10 minutes after the daemon starts and then
every 24 hours; updates are never installed automatically.

### `Remove(s repo, b uninstall)`

Removes a repository with its cached previews; `uninstall` also uninstalls its packages
(`Failed` if one of them is shown on an output). Without it the packages stay usable, without
updates or revocations. Queued or running jobs of this repository (its refreshes and installs
from it) are cancelled first and the call waits only for them; jobs of other repositories go on.
An install from it that starts while it is being removed ends `cancelled`.

### `Browse(s query, u offset, u limit) → (s json)`

A `BrowsePage` of the packages offered, matched without case on id, title, description (in every
language), tags, authors and facets, sorted by id then repository. `limit` is `0..=500`. Titles,
descriptions and change notes come in the user's language; with `library.hide_mature`, packages
rated `mature` are left out.

### `RequestPreviews(as specs) → (a{ss} paths)`

Returns `{spec: path}` for the cached previews of `[repo/]id[@version]` specs (at most 500,
keys as given) and queues the others; `PreviewReady` follows for each of them. A new call from
the same sender replaces its queued requests. Previews are downloaded within their signed size
and hash, re-encoded by the thumbnail worker to JPEG files of at most 1024 px in
`~/.cache/wallpaper-rs/previews/`, and only the re-encoded file is handed out.

### `Install(s spec, b allow_stale, b allow_downgrade) → (t job)`

Installs `[repo/]id[@version]`; without a version, the newest release when it is newer than the
installed one (this is how updates are installed), else the job finishes at once with the
installed record. Checked before the job starts: `InvalidArgs` for a malformed spec, a package
offered by several repositories without `repo`, a package installed from another repository or an
explicit version older than the installed one without `allow_downgrade`;
`NotFound` for an unknown package or version; `Failed` for an expired root (`root expired:
possible freeze attack`) or a revoked version; `Stale` when the repository's index expired and
`allow_stale` is false. The first installed version is imported into the library; later
versions relink the same item, keeping its title, tags, rating, presets and playlist entries.

### `Uninstall(s id, s version)`

Uninstalls one version, or every version for `version = ""`. `Failed` when one of them is shown
on an output. When the version linked to the library item goes, the item moves to the newest
remaining version, or is removed with its thumbnails when none remains.

### `ListInstalled() → (s json)`

An `InstalledReply`; `item` is set on the version linked to the library item and `update` on
the newest installed version when a newer one is available, with `changes` holding the change
notes of that version in the user's language and with its markup. `RefreshOutput.updates` carries
the same notes.

### Signal `PreviewReady(s spec, s path)`

A preview queued by `RequestPreviews` is ready, or could not be made (`path = ""`: unknown
package, no preview, or a failed download or check).

## `io.github.wallpaper_rs.Inputs1`

Scenes may follow the sound that plays and the pointer. Both inputs are off for every scene until
the user grants them to that scene; grants are kept per scene package id in
`~/.local/share/wallpaper-rs/inputs.json` (mode `0600`) and can be revoked at any time. A scene
never follows the sound it plays itself: sound layers cannot take their values from the analysis. A
still image whose depth parallax has `source` `pointer` (see "Edits and spanning") also uses the
pointer: choosing that source for that image is the user's consent, so it needs no grant, and it
follows the same rules as a scene, without clicks.

An input is on only while an output shows a scene that declares it and holds the grant (or a
pointer parallax image that moves: not under reduced motion, nor with software rendering unless
`playback.software_parallax` is set), the screen is unlocked (whatever `pause.when_locked`
says) and that output is active (not paused, and not covered while `pause.when_covered` holds);
the pointer also stops while the output is covered. Revoking, locking, pausing or hiding the
scene or image stops it at once.

- Audio: the daemon captures the monitor of the default output device through a PipeWire
  stream named "wallpaper-rs audio analysis" (desktop microphone indicators may show it) and
  turns it into at most 64 spectrum bands per channel, levels and beats on the render thread.
  Samples stay in memory only for that analysis: they are never stored, logged or sent anywhere,
  and scenes and scripts only get the bands and levels. A scene without the grant gets silence
  even while another output's scene is listening. The stream is destroyed when no scene needs it.
- Pointer: on layer-shell outputs only the background of an output whose scene or parallax image
  uses the pointer accepts pointer events, showing the default cursor, so other backgrounds never
  take desktop clicks; in Plasma the daemon loads a KWin script that reports
  `workspace.cursorPos` with `SetPointer`, at most 60 times per second and only while the
  pointer moves, and unloads it when no scene or parallax image needs it. Keys are never read,
  and positions are neither kept nor logged: each output only has the latest one.

### `Grant(s scene, s input)`

Lets the scene with package id `scene` use `input`, `audio` or `pointer`, whenever it is shown.
`InvalidArgs` for another input or a malformed id (1 to 255 characters of `a-z`, `0-9`, `.`,
`_`, `-`). Granting twice changes nothing.

### `Revoke(s scene, s input)`

Takes `input` back from `scene` and stops it at once if that scene was using it. It also records
that the user declined `input` for `scene`, in `inputs-declined.json` next to `inputs.json` (mode
`0600`), so that clients stop asking; `Grant` clears that record.

Clients ask once per scene: when `shown` lists an input that the scene has neither been granted
nor declined, whether the scene was applied from a client, a playlist or `wprs-ctl`, the app and
the tray explain what is captured and that nothing is stored or sent, and offer *Allow*
(`Grant`) or *Not now* (`Revoke`); the scene settings show the same hint with these actions until
the user decides. `wprs-ctl lib apply`, `playlist start`, `next` and `prev` follow the same rule:
once the daemon lists the inputs of the scene they showed in `shown`, they print the
`wprs-ctl input grant` command for each input that is neither granted nor declined.

### `GetInputs() → (s json)`

An `InputsReply`: the grants, the declined inputs (`declined`), the scenes that declare inputs on
each output with their `title`, the outputs whose
parallax image follows the pointer (`parallax`), whether audio is being captured (`capturing`)
and whether the pointer is being followed (`tracking`).

### `SetPointer(i x, i y)`

The pointer position in logical desktop coordinates, as sent by the KWin script. It is placed
on the output whose `SetOutputInfo` geometry contains it when that output follows the pointer, and
ignored otherwise.

## `io.github.wallpaper_rs.Sync1`

Connects this computer to one self-hosted `wprs-server` (see `server.md`) and keeps its library
metadata, playlists, profiles, presets, settings, added repositories, chosen provider secrets and
chosen wallpaper files in sync with the other devices of the account. The server address is `https://`, or `http://` to this computer, without user
information, query or fragment; the daemon contacts no other host for sync, follows no redirect
and sends the generic `User-Agent`. Every answer is limited in size (1 MiB, 8 MiB for pages of
changes, 2 MiB for the mailbox, 64 KiB per event); a larger one fails the job without being used.
The device token, the private X25519 key of this device, the sync key (a 32-byte record key of
the current key epoch and a 32-byte naming key) with its epoch, the next key during a key
rotation and the devices that still wait for it, the key that seals the recovery escrow and the
synced copies of `secrets` records live
only in the Secret Service, in one item labelled `wallpaper-rs: server` with the
attributes `xdg:schema = io.github.wallpaper_rs.Server` and `server = server`, with the timeouts
and unlock rules of `Providers1`; they never appear in replies, signals, job targets, job errors,
logs, `config.toml` or command lines. The rest of the state (server address, device names, sync
heads, the last synced copy of every other record, values kept for other versions, the choices of
private files, a running comparison and the public keys of the devices this computer compared
codes with) is kept in
`~/.local/share/wallpaper-rs/sync.json` (`0600`).

End-to-end encryption is the default: every record is `0x01 ‖ nonce ‖ XChaCha20-Poly1305` of the
JSON padded to a multiple of 256 bytes (a `0x80` byte, then zeros), with the key and the
additional data of `server.md` (user id and epoch as 8-byte big-endian integers, epoch also
8-byte big-endian in the HKDF info). Plain-text sync exists only as the explicit opt-out
`Link(plaintext = true)` on an account that has no sync yet; the server then labels the account
with a warning, reported in `warning`. The setting is fixed when the device links: a sync fails
without sending anything when the server later reports another one. In both modes the naming key
`K_id` of `server.md` names the items: library items are `hex(HMAC-SHA-256(K_id, repo_id ‖ 0x00 ‖
package id))` for installed packages and `hex(HMAC-SHA-256(K_id, "blob:" ‖ quick hash))` for other
files, so devices that hold the same file share its record; playlists, profiles and presets are the
first 32 hex digits of `HMAC-SHA-256(K_id, kind ‖ 0x00 ‖ uid)` and settings sections of
`HMAC-SHA-256(K_id, "section" ‖ 0x00 ‖ name)`, so the server cannot tell sections, profiles and
presets apart. Records synced by older versions under the plain names `settings/<section>`,
`settings/profile_<id>` and `settings/preset_<id>` move to these names on the first sync, and the
old records are deleted. The server names devices `Device N`; the name of this computer
(`Link(name)`, the host name when empty) travels only inside the encrypted `device` record and is
never sent in plain-text mode. Devices linked by older versions pin, once, the devices whose
synced `device` record names the key the server lists.

| Collection | Item | Fields |
|------------|------|--------|
| `library` | keyed hash | `title`, `rating`, `favorite`, `t:<tag>`, one field per part of the edit of a picture, so parts changed on two devices both survive: `e:geometry`, `e:ops`, `e:masks`, `e:span`, `e:depth`, `e:parallax`, `e:fit` and `e:outputs/<screen>` for each screen crop, leaving out the `source` and parts at their default (a part over 64 KiB keeps the last copy sent, parts this version does not know are kept as synced, and a received edit keeps the `source` of this computer) and `blob` `{id, hash, size, name}` for a private file on the server; only items with a tag, a rating, a favorite mark, an edit or a private file get a record |
| `playlists` | keyed hash | `name`, `query`, `r:<rule>` (rules this version does not know are kept as synced), and for manual playlists one `e:<entry>` per entry with its item and a fractional position |
| `settings` | keyed hash of the section | every option except `playback.volume`, `playback.muted`, `playback.software_parallax`, `library.thumb_cache_mb` and `models.allow_download`; options and sections this version does not know are kept as synced |
| `settings` | keyed hash | `kind` (`profile` or `preset`) and the profile or preset, with items, playlists and presets named by their synced ids; the options of a profile leave out the settings above that stay on each device, and a received profile keeps their values from this computer |
| `device` | device id | `name`, `platform`, `key` (base64 of the public X25519 key); never applied, shown in `devices[].label` |
| `repos` | keyed hash of the repo id | `url` (with its trust anchor), `repo_id`, `name`, `fingerprints` of the repositories added with an `https://` or loopback `http://` URL; a new one is only offered (`offers`) until `ConfirmRepo` |
| `secrets` | keyed hash of the provider id | `provider`, `kind`, `bind` (its `url`, `server` and `user`), `secret`; only with end-to-end encryption and for providers chosen with `ShareSecret` |

Every record holds per field a value and a hybrid logical clock (milliseconds, counter, device);
a sync pulls the changes since the last head, merges each record field by field with the later
clock winning (a deletion wins only when it is later than every field), applies the result,
pushes local changes with the `seq` they are based on and repeats the round for conflicts at
most three times. Records whose clock is older than the last one seen for the item are refused
and overwritten with the newer copy. After `Join` or `Restore`, or when the server answers `410
resync`, the sync downloads every record and uploads again the ones the server no longer holds. Devices delete with a record whose payload says so; a
`deleted` record without payload carries no clock, nor in an encrypted account any proof of its
origin, so it deletes nothing and the devices upload their copy again.
Library records of items that are not on this computer,
playlist entries and presets of such items are kept and applied once the item appears. A record
that cannot be decrypted fails the first sync that meets it with the count of such records and is
then skipped (`quarantined`) until the server holds a newer copy of it.

A page of changes, or a change, of another key epoch than the one the sync started with stops the
sync with `epoch_mismatch` before anything is stored, and the next sync takes the new key first;
records of the current or an older epoch that cannot be decrypted are quarantined.

Fields this version does not know, and values it does not understand inside fields it knows (an
option value, a playlist rule value, a rating above 5), are kept exactly as received: the field
keeps its local value, and the copy sent back carries the received value until the user changes
that field here. A received change that cannot be applied here (for example a profile name that
cannot be stored) is not counted as synced: the record keeps the state this computer shows, the
change is tried again at every sync, nothing is sent back for it, the first sync that meets it fails
with the count and the first reason, and `unapplied` counts such changes. A received name of a
playlist, profile or preset that another object already uses here (or that a preset never takes)
keeps the local name, and the copy sent back carries the received name until the name changes here.
A repository that already has a record keeps the record's address, name and fingerprints in the
copy sent back until they change on this computer, so devices that added it through different
addresses do not overwrite each other.

When two devices change a playlist in ways that do not merge (one deletes it, or switches it
between manual and smart, while the other changes its entries), the side with more entries wins
and the other side is kept as a new playlist named `<name> (conflict)`, the suffix in the language
of the daemon (`LC_ALL`, `LC_MESSAGES` or `LANG`); a deleted side leaves no copy.

A repository removed on one device is removed on the others too, keeping its installed packages.
Provider secrets travel only inside end-to-end encrypted records and are stored on a receiving
device only in the Secret Service item of a provider with the same id, kind, `url`, `server` and
`user`; they are never written to `sync.json`, and their synced copies live in the server keyring
item. Only a device where `ShareSecret` chose a provider sends its secret; the others apply the
received one when the provider matches and never send their own. A received record without the
secret, or with a secret for another `url`, `server` or `user`, ends the choice of this device, so
the last device that chose wins.

Private files are files of the library that come from no repository: `ShareFiles` chooses items and
folders, and a `files` job (kind `upload`) encrypts each file and uploads it to
`/api/v1/sync/blobs` (see `server.md`) in pieces of 64 segments, resuming after the last piece the
server holds; identical files are stored once. Each 64 KiB segment is sealed with
ChaCha20-Poly1305 under `HMAC-SHA-256(K_id, "wprs-sync/file-key/v1" ‖ SHA-256 hex of the file)`, with
a nonce of the segment number (11 bytes big-endian) and a byte that marks the last segment; the
server name of the file is `hex(HMAC-SHA-256(K_id, "stored:" ‖ SHA-256 hex))`. Other devices list
them with `ListFiles` and download one on demand with `FetchFile`, which checks every segment and
the SHA-256 of the whole file and imports it into `~/.local/share/wallpaper-rs/files/`. A sync
starts the `files` job when private files are chosen; it also deletes files that this computer
uploaded and that no record refers to any more for a day, only while its copy of the records is as
new as the server's. Transfers run in their own queue, so syncs and key changes never wait for
them.

Before syncing, the daemon compares the key epoch and key id of `/api/v1/sync/state` with its key.
When the account moved to later epochs, it takes each new key in turn from a `rotate` message of a
pinned device: a device whose public key this computer learned in a comparison with matching codes
(`Join`, `ConfirmShare`), never one the server lists; a device that leaves the device list is
unpinned for good. Messages of later epochs and of other devices stay in the mailbox; older ones
and those that do not open are deleted. Without such a message the state becomes `needs_key` and
nothing is sent until `Join`. After taking a key it passes it on to its other pinned devices, so
the key follows the comparisons from device to device; a message that cannot be sent is sent again
at every sync until it arrives. The daemon replaces the key itself (`Rotate`) when the server
reports `rotate` after a device was revoked, or when a device listed at the last key change has
left the device list, and it resumes an interrupted rotation with the next key kept in the Secret
Service. A device that missed a revocation still trusts the revoked device if it compared codes
with it; join such a device again.

While a linked device holds the sync key, or waits for the next one, and sync is not paused, the
daemon keeps the event stream of the server open (reconnecting after 30 s, doubling up to 15 min),
syncs when another device wrote, when the key epoch changes, when a message arrives while it waits
for a key, 5 s after local library, playlist and profile changes that differ from the synced copies
(showing a wallpaper is no such change), every 15 minutes and at startup. Automatic syncs never show an unlock prompt; a locked keyring sets `last_error`.

### `AddServer(s url) → (t job)`

Starts an `account` job that reads `/.well-known/wprs-server` and stores the server. `Exists`
when a server is already added; `InvalidArgs` for a refused address. The repository the server
announces is remembered when its address has the origin of the server.

### `Link(s name, b plaintext) → (s user_code, s verification_uri, t job)`

Creates the key pair of this device, starts the device code flow with the platform, the public key
and the client name `wallpaper-rs/<version>`, and returns the code to approve at
`verification_uri` (or with `Approve` on a linked device); an address the server gives on another
origin is replaced with `<server>/link`. The `link` job polls until the code is approved, refused
or expired (10 minutes), stores the token, sets up sync with the key id of a new sync key when the
account has none (encrypted unless `plaintext`, then the key is stored first), adds the server's
repository with `Repos1.Add` and syncs. On an account that already syncs, the device
needs the sync key from another device: state `needs_key`, see `Join`. When the account syncs
without encryption and `plaintext` is false, the job fails, asks the server to revoke the new
device and forgets its token. `NotFound` without a
server, `Exists` when this computer is linked or linking, `InvalidArgs` for a name over 64
characters or with control characters.

### `PreviewCode(s user_code) → (t job)`

Shows what waits behind a code before approving it: an `account` job whose output is the
server's `{platform, client, requested_at, expires_at}` of the pending request (times in Unix
milliseconds), never an address. Clients show it with the warning to approve only a code that the
user's own new device shows, and ask for an explicit confirmation before `Approve`.

### `Approve(s user_code) → (t job)`

Approves the code shown by a new device of the same account from this linked device.

### `GetStatus() → (s json)`

An `AccountReply`; `account` is `null` without a server. It reads no network; `devices` and the
quota are those of the last sync. `pairing` is the running comparison of `Join` or `ShareKey`: the
other device's id, the code once both nonces are known, and `sharing` on the device that holds
the key. `quarantined` counts the records skipped because they cannot be decrypted and `unapplied`
the received changes that wait to be applied. `offers` are repositories synced from other devices
that wait for `ConfirmRepo`, `secrets` the providers whose secret is synced, `recovery` whether the
server holds an escrow of the current key (`current`), of an older one (`stale`) or none (`off`),
and `files_bytes` of `files_quota_bytes` the private files on the server.

### `SyncNow() → (t job)`

Starts an `account_sync` job. `Failed` until this computer is linked and holds a sync key.

### `SetPaused(b paused)`

Pauses or resumes automatic sync and closes or opens the event stream; `SyncNow` still works.

### `Join(s device) → (t job)`

On a linked device without the current sync key: `device` is the id, server name or label of a
device that holds it, which runs `ShareKey` for this device. The `link` job runs a
commit-then-reveal comparison through the mailbox, with `key-share` messages whose body is the
JSON `{v: 2, step, pair, data, epoch}` (`data` base64); a message of another version, of another
comparison, out of order or replayed fails the job, and so does an `abort`. This device sends
`commit` with a random 16-byte comparison id `pair` (32 hex digits) and `SHA-256("wprs-sync/commit/v2"
‖ pair ‖ N_j)` for a random 32-byte nonce `N_j`, waits for the other device's `nonce` `N_h`, and
only then reveals `N_j`. Both devices show `GetStatus().account.pairing.code`: six digits of
`T = SHA-256("wprs-sync/pair/v2" ‖ pair ‖ joining public key ‖ holding public key ‖ N_j ‖ N_h)`
(the first 8 bytes big-endian modulo 10⁶, as `123 456`), so a server that changes the public keys
gets one guess per comparison. The comparison is mutual: the job waits both for the `key` message
and for `ConfirmShare` on this device with the code the other device shows, keeps a received key
only in memory until then, and sends `abort` when the codes differ. It then opens the key with
HPKE (auth mode, DHKEM X25519, HKDF-SHA256, ChaCha20-Poly1305, info `wprs key-share v2`,
additional data user id ‖ receiving device id ‖ epoch ‖ one byte, 1 when the account is encrypted
and 0 otherwise ‖ `T`), checks the key id the server reports, stores the key and pins the other
device, so a server that plays the other device, or tells the two devices different accounts,
epochs or encryption settings, makes the share fail. Each step waits at most 10 minutes.

### `ShareKey(s device) → (t job)`

On a device that holds the current sync key: a `link` job that waits up to 10 minutes for the
`commit` of the joining `device`, answers with its own `nonce`, checks the revealed nonce against
the commitment and outputs `{device, code}`; the code also stays in `pairing` for 10 minutes.

### `ConfirmShare(s device, s code) → (t job)`

Confirms the running comparison with `device`; `code` is the six digits that `device` shows. On
the device that holds the key it starts an `account` job that sends the sync key sealed, to the
public key used in the comparison, in a `key` message when `code` equals the code of the
comparison, and pins `device`. On the joining device it returns the job of the running `Join`,
which stores the key only when `code` matches. Every comparison allows one attempt on each side:
when the codes differ the job fails with `InvalidArgs`, which means the server or another device
stands between them, and the other device gets `abort`.

### `Rotate() → (t job)`

Replaces the sync key of an encrypted account in an `account` job: makes the next record key
(the naming key stays), stores it as the next key, starts the next epoch with its key id, encrypts
every record the server holds again (records of the old epoch that cannot be decrypted and
deletions without payload are not kept), finishes the epoch, which deletes the remaining old records, and sends every pinned device a
`rotate` message: the new key sealed to its pinned public key like a `key` message with the
additional data ending in `HKDF-SHA256(old record key, "wprs-sync/rotate/v1" ‖ epoch)` instead of
`T`. Pinned devices pass it on to theirs; other devices get the key with `Join`. A rotation after
a device left forgets the key of the recovery escrow (`recovery` becomes `stale` until `SetEscrow`
runs again); otherwise this device seals the new key in the escrow again when it holds that key.
Output: `{epoch, records, shared}`; messages that could not be sent are sent again by later syncs.
It fails when another device replaced the key first.

### `Reset() → (t job)`

For an account whose key holders are lost: deletes every synced record on the server, sets up
sync again with a new record key (the naming key and the links between records and local objects
stay, so devices that join again merge instead of doubling their objects) with end-to-end
encryption unless this device linked with `plaintext` to a plain-text account, and uploads the
library, playlists and settings of this computer; output as `SyncNow`. Clients ask for an explicit
confirmation first. Every other device has to `Join` again.

### `ConfirmRepo(s repo, b add) → (t job)`

Decides on a repository that another device synced and that waits in `offers`; `repo` is its offer
`id` or at least the first 8 digits of its `repo_id`. With `add`, returns the `repo_add` job of
`Repos1.Add` with the synced URL, its trust anchor completed with `repo_id=`; without it the offer
is declined on this computer, returns `0`, and is not offered again. `NotFound` when no such offer
waits, `InvalidArgs` when the URL names another repository than the record.

### `ShareSecret(s provider, b share)`

Starts or stops sending the secret of a provider of a kind with a secret from this device, only on
an encrypted account (`Failed` otherwise, `InvalidArgs` for a provider without secret). Stopping
removes the secret from the synced record once and keeps the copies that other devices already
hold.

### `SetEscrow(s passphrase) → (t job)`

Stores the sync key on the server for recovery, in an `account` job: `passphrase` of at least 20
characters is turned into a key with Argon2id (256 MiB, 3 passes, one lane, a random 16-byte salt)
that seals the record and naming keys with XChaCha20-Poly1305; the escrow is `0x01 ‖ memory (KiB)
‖ passes ‖ salt ‖ epoch ‖ nonce ‖ sealed keys` (numbers big-endian, 4 bytes for the costs and 8 for
the epoch) with the additional data `"wprs-sync/escrow/v1" ‖ user id ‖ the first 33 bytes`. The
derived key stays in the Secret Service item of this server, so that after a `Rotate` of this
device it seals the new key again; a different escrow on the server makes it forget that key, and
so does a key change after a device left, because the revoked device may know the passphrase: set
the escrow again then, with a new passphrase, since whoever knew the old one can open every escrow
sealed with it. An empty passphrase deletes the escrow. Only on an encrypted account. Clients warn
that whoever holds a copy of the server's data can try passphrases without limit.

### `Restore(s passphrase) → (t job)`

On a linked device without the sync key: a `link` job that opens the escrow with `passphrase`,
refuses an escrow of an epoch older than one this computer already saw, checks that it holds the
key of the current epoch and key id, and stores it like `Join`; the output names the epoch. A
device that got the key this way pins no device, so it needs `Join` or `Restore` again after a key
change.

### `ShareFiles(as targets, b share) → (t job)`

Chooses private files: each target is an item uid or an absolute folder path (every item below it
is chosen); with `share = false` an item or folder is unchosen, which removes its file from the
server, and a target can also be the `id` of a file in `ListFiles` that is not on this computer.
Returns the `files` job. Only on an encrypted account; `NotFound` for an unknown item, `InvalidArgs`
for a missing folder.

### `ListFiles() → (s json)`

A `FilesReply`: the chosen folders and every private file, those on the server (`stored`, with the
local item when this computer has the same file and `shared` when it is chosen here) and those
waiting to upload. It reads no network.

### `FetchFile(s file) → (t job)`

Downloads the private file `file` (its `id` in `ListFiles`) in an `upload` job whose output is
`{item}`, resuming a partial download.

### `Upload(s path) → (t job)`

Uploads a `.wpak` package with this device's token (`POST /api/v1/uploads`, `PUT`, `finalize`)
in an `upload` job whose output is an `UploadOutput`; the job fails when the server refuses the
package. It is published in the server's repository after review, or at once when the server
publishes uploads of the account's role.

### `RemoveServer(b wipe)`

Cancels the account jobs, deletes the keyring item of this server even when linking stopped half
way (`KeyringUnavailable`: nothing is removed), asks the server in the background to revoke this
device, and forgets the server. Without `wipe`
the synced copies stay so that linking the same account again resumes; `wipe` deletes them too.
The added repository stays.

## The app: `io.github.wallpaper_rs`

The desktop app owns `io.github.wallpaper_rs` (`APP_ID`) at `/io/github/wallpaper_rs` (`APP_PATH`)
and implements `org.freedesktop.Application` there. Its desktop file sets `DBusActivatable=true`
and `packaging/dbus/io.github.wallpaper_rs.service` lets the session bus start it for any of these
calls. A second `wprs-app` passes its files and page to the running one through the same calls
and exits.

| Method | Effect |
|--------|--------|
| `Activate(a{sv} platform_data)` | raises the window |
| `Open(as uris, a{sv} platform_data)` | imports the `file://` URIs and absolute paths into the library, then raises the window |
| `ActivateAction(s name, av parameter, a{sv} platform_data)` | `library`, `playlists`, `profiles`, `providers`, `repos`, `settings` or `account` opens that page, also in an app the bus starts for this call; any other name only raises the window |

A request for the page already shown keeps it as it is, and while the image editor has unsaved
changes a page request only raises the window. `wprs-app --page <name>` and the launcher action
*Settings* (`settings`) make the same request. The tray offers *Open wallpaper-rs* (`Activate`)
and *Settings…* (`ActivateAction("settings")`) unless `ListActivatableNames` lists the daemon but
not the app and the app is not running: the Flatpak ships no app, so there the tray says that the
other settings are changed with `wprs-ctl`.

## Edits and spanning

Edits never change the image. An edit lives in `<image file name>.wprs.json` next to the image
(`hills.jpg.wprs.json` for `hills.jpg`) or, when that folder cannot be written, in
`~/.local/share/wallpaper-rs/edits/<SHA-256 of the canonical image path>.json`, which is also
used when the file next to the image is a symbolic link; when both exist the newer one wins. Both
are written atomically with mode `0600` and read up to 2 MiB. `wprs-ctl edit` changes them without
the daemon. The daemon reads the edit whenever it loads an image for an output (a file, a library
item, a package entry or a playlist item) and loads it again as soon as the file changes; an edit
that cannot be used is ignored with a warning.

```json
{
  "version": 1,
  "source": { "name": "hills.jpg", "bytes": 8421337, "sha256": "9f2c…", "pixels": [6000, 4000] },
  "geometry": { "crop": [0.0, 0.08, 1.0, 0.84], "rotate": 0, "straighten": -1.2, "flip": [false, false] },
  "ops": [
    { "id": "wb", "type": "white_balance", "temperature": 5600, "tint": 4 },
    { "id": "ev", "type": "exposure", "ev": 0.35 },
    { "id": "bl", "type": "blur", "radius": 18, "mask": "back" },
    { "id": "ov", "type": "overlay", "color": "#1a2b6d", "blend": "soft_light", "opacity": 0.35, "mask": "sky" }
  ],
  "masks": {
    "back": { "type": "luminance", "range": [0.0, 0.35], "soft": 0.05 },
    "sky": { "type": "linear", "from": [0.5, 0.0], "to": [0.5, 0.45], "invert": true }
  },
  "outputs": { "Maker Model 1234": { "connector": "DP-1", "crop": [0.0, 0.1, 0.5, 0.5625] } },
  "span": { "center": [0.5, 0.46], "zoom": 1.0 },
  "depth": { "model": "depth-anything-v2-small", "input": 518, "near": 1.0, "far": 0.0, "gamma": 1.0, "smooth": 2, "dilate": 3 },
  "parallax": { "enabled": true, "source": "auto", "strength": 0.02, "focus": 0.3, "steps": 16, "smoothing_ms": 150, "overscan": 1.04 },
  "fit": "fill"
}
```

- `version` is `1`; newer files are refused. Unknown fields, out-of-range values and broken
  references are errors that name the field, such as `ops[1].ev: 7 is out of range -5 to 5`.
- `source` holds the name, size, SHA-256 and pixel size of the image when the edit was created;
  `wprs-ctl edit` warns when the file size or the pixel size changed.
- `geometry`: `crop` `[x, y, w, h]` in fractions of the rotated image (`null` for all of it),
  `rotate` clockwise `0`, `90`, `180` or `270`, `straighten` from `-45` to `45` degrees
  counterclockwise keeping the largest centred rectangle, `flip` `[horizontal, vertical]`.
- `ops` run in order, at most 24. Each has a unique `id` (1 to 64 letters, digits, `-` or `_`), a
  `type`, `enabled` (`true` when absent), an optional `mask` naming one of `masks` and an
  optional `opacity` from `0` to `1`.

| `type` | Fields |
|--------|--------|
| `exposure` | `ev`, `-5` to `5` |
| `white_balance` | `temperature`, `2000` to `12000` K with `6504` neutral; `tint`, `-100` to `100`, positive adds magenta |
| `contrast` | `amount`, `-100` to `100` |
| `saturation` | `saturation` and `vibrance`, `-100` to `100` |
| `curves` | `rgb`, `r`, `g` and `b`: empty or 2 to 16 `[x, y]` points from `0` to `1` with growing `x` |
| `blur` | `radius`, `0` to `200` pixels of a picture 1000 pixels long |
| `overlay` | `color` `#rrggbb`; `blend` `normal`, `multiply`, `screen`, `overlay`, `soft_light` or `color` |
| `vignette` | `amount`, `-1` to `1`; `midpoint`, `roundness` and `feather`, `0` to `1` |

- `masks`, at most 8, use coordinates of the original image: `linear` (`from`, `to`), `radial`
  (`center`, `radius` `[rx, ry]`, `feather`), `luminance` and `depth` (`range` `[low, high]`,
  `soft`; `depth` reads the depth map, `1` near, and selects nothing while the image has none)
  and up to 4 `brush` masks (`strokes` of `radius`, `hardness`, `erase` and `points`, at most
  20 000 points in all). Every mask takes `invert`.
- `outputs` crops the edited image for one screen, keyed by its identity (as in
  `SetOutputInfo`) or output name; `connector` names the output as a hint. The Android app keeps
  the framing chosen on the phone under `Android`, in the shape of the phone's screen, inside the
  `geometry` crop; computers leave it to the phone.
- `span` places the image when the fit is `span`: `center` `[x, y]` and `zoom` from `1` to `8`.
- `depth` shapes the depth map: `near` and `far` (`0` to `1`) are the levels of the nearest and
  farthest points, `gamma` (`0.2` to `5`) curves the levels between them, `dilate` (`0` to `16`
  pixels of the map) grows near objects over their edges and `smooth` (`0` to `16`) blurs the
  result. `model` is `depth-anything-v2-small` and `input` (a multiple of 14 from `196` to `1036`)
  the short side given to it, which also names the generated map.
- `parallax` moves the image in depth on the desktop while `enabled`: `source` is `auto` (a slow
  orbit), `pointer` (the pointer feed of `Inputs1`, back to the centre when the pointer leaves the
  outputs that show the image or rests for 3 s) or `accelerometer` (the IIO accelerometer of the
  computer, `(raw + offset) × scale` turned by its mount matrix, read 30 times per second only
  while the parallax is shown, looked for again every 5 s while none answers, with its resting
  tilt slowly recentred); a new source starts from the offset the image has;
  `strength` (`0` to `0.03`) is the largest shift as a fraction of the image width, `focus` (`0`
  far to `1` near) the depth that stays still, `steps` `1`, `8`, `16` or `32` the quality,
  `smoothing_ms` (`0` to `2000`) the time a critically damped spring takes to follow the source and
  `overscan` (`1` to `1.1`) the zoom that hides the edges.
- `fit` (`fill`, `fit`, `stretch`, `center` or `tile`) shows this picture with its own fit instead
  of the fit of the output, except on `span` outputs; `GetState().outputs[x].fit` stays the fit of
  the output and `shown_fit` reports the fit in use.

The depth map of an image is a grey PNG, white near. It can sit next to the image
(`hills.jpg.depth.png`, where `wprs-ctl edit depth import` and `export` put it), in
`~/.local/share/wallpaper-rs/depth/<SHA-256 of the image>.png` (imports whose folder cannot be
written) or in `~/.cache/wallpaper-rs/depth/<SHA-256>-<model>-<input>.png`, where
`wprs-ctl edit depth generate` and the app write maps estimated in their own process; the newest
of these files wins, and on equal times an import comes first. An import whose width to height
ratio differs from the image's (after its EXIF orientation) by more than 2 % is refused. The cache
keeps at most 256 MB of estimated maps and deletes the oldest first; temporary files left by an
interrupted write or install are deleted by the next one in that folder once they are an hour
old. The daemon never runs the model: it reads the map at
1024 pixels at most, applies `depth` and reloads it when any of these files changes. A parallax
image is rendered only while its smoothed offset moves by a quarter of a pixel or more, at most
at the frame rate of the output, under the same pause rules as videos (`pause.when_covered`,
`pause.on_battery`, `pause.when_locked` and `SetPaused`), and stays still while the desktop asks
for reduced motion (`reduced-motion` of the Settings portal). With software rendering (a GPU
emulated on the CPU) the image stays still and flat unless `playback.software_parallax` is set,
and then moves with 8 steps at most and 15 frames per second at most. On `span` outputs every
screen moves by the same offset; `tile` images stay still. The model and ONNX Runtime are only
downloaded by `wprs-ctl models download` or the app after showing their size, license and every
host they may come from, from their pinned address (the manifest may list mirrors, which must
serve the same bytes), with a generic User-Agent, no Referer, no redirect from HTTPS to HTTP and
their pinned SHA-256; `NOTICE.txt` and the full license text (`LICENSE.txt`, shipped with
wallpaper-rs) are stored next to each file, and ONNX Runtime keeps the `THIRD-PARTY-NOTICES.txt`
of its archive. ONNX Runtime runs with its telemetry off (`ORT_DISABLE_TELEMETRY=1` and its
environment's switch), so it neither stores a device id nor contacts anyone.
`models.allow_download = false` turns downloads off on this device only: it is not synced and a
configuration that cannot be read counts as `false`, also when a setting change saves it again.
`wprs-ctl models install` takes files fetched by other means.

Exposure, white balance, contrast, saturation, vignette and blur work in linear light, curves and
overlays (with their `opacity`) in sRGB. The GPU renders an edit once, at the size the output
needs, and dithers the result; nothing is rendered again while the image, the edit and the output
stay the same.
`wprs-ctl edit export` renders with the same code at the size of the edited image, or at `--size`
after cropping the edited image to that shape.

With the `span` fit, the outputs that use it form one canvas in millimetres. A screen measures its
physical pixels divided by its density (96 dpi times its scale when the density is unknown or
outside 2 to 20 px/mm). Screens that touch in the logical layout touch on the canvas, centred along
the shared edge and `2 × layout.bezel_mm` apart; `[layout.monitors."<identity or output>"]` with
`mm = [x, y, w, h]` places a screen by hand. The edited image covers the canvas, moved by
`span.zoom` and `span.center`, and every output shows its part of it, so a picture keeps its
physical size across screens of different densities. An output without `SetOutputInfo` shows
`span` like `fill`.

## Scenes

A scene package (`kind: scene`) holds a `scene.json` (see `scene.md`) and the assets it names.
The daemon renders it on the GPU for every output that shows it, with the same streams, fits of
the scene canvas, transitions and claims as other wallpapers.

- A frame is rendered only when something visible changes: a running animation or timeline, a
  clock text when its minute (or second) changes, a video frame, a changed user property, or an
  input the scene uses (audio levels, pointer, script outputs). A still scene renders once and
  then only the 10 s keep-alive of the stream runs.
- The frame rate is the lowest of `playback.max_fps`, the scene's `clock.max_fps`, the quality
  level and the GPU class, as described in "Frame rate"; a scene with `clock.max_fps = "display"`
  follows the refresh rate of each output.
- The quality level follows the GPU class and the power source: on battery or on a software
  renderer heavy effects run at a lower resolution or not at all when the scene marks them
  `optional`, and the scene itself renders at 0.75 or 0.5 of the output size.
- An output that is not active (paused, covered, locked or paused on battery, see
  `SetOption`) or whose stream has no consumer renders nothing and its scene clock stops; it
  continues where it stopped, without catching up.
- `GetState().outputs[x].kind` is `scene` and `playing` tells whether the scene is animating.
- `GetState().outputs[x].sound` tells what the sound of the scene does there (see "Scene sound").
- Example scenes are packages in `wallpaper-rs/examples/` under `$XDG_DATA_HOME` or a folder of
  `$XDG_DATA_DIRS`. The daemon adds each one to the library once, when it starts and on a
  `Rescan` of every source; an example removed from the library is not added again, and the
  item of an example whose file was uninstalled is removed. `kind:scene` finds scene packages.

## Scene sound

The sound layers of a scene (see `scene.md`) play through the same audio path as videos (see
`SetVolume`), with the same handling of a missing audio output. The daemon mixes all the layers of
a scene into one stream, so a scene has one audio pipeline at most. When that pipeline is first
created, one worker thread decodes into memory the sounds of the layers that can play (not those
whose `play` is a fixed `false`), the playing ones first, once for each version of the package:
later pipelines of the same package reuse them, and they are freed when no output shows it. Each
sound is decoded up to the length that its header gives plus 0.5 s, and all the sounds of a
package up to 180.5 s; a sound that goes on longer is not played.

- Scenes start muted like videos. Their sound plays only while `playback.muted` is `false` and the
  stream volume, `playback.volume × volume / 100`, is above 0, where `volume` is the user property
  that every scene with sound has (a slider from 0 to 100 %, 100 by default, added to
  `GetSceneProperties` unless the manifest declares it as such a slider itself).
- The pipeline is created only while the scene is shown on an active output (not paused, covered
  or paused on battery, and with a consumer for a stream) and the screen is unlocked. It pauses
  exactly when the scene pauses, together with the scene clock, and goes on from the same place;
  it is destroyed when the scene leaves the output and when the sound is muted or turned down to
  0. Once every sound layer has stopped and faded out it pauses, and 5 s later it is destroyed; a
  layer that starts again within these 5 s resumes it, and one that starts later creates it again.
  Nothing plays while the screen is locked or the output is covered, whatever `pause.when_locked`
  and `pause.when_covered` say; videos are muted while the screen is locked too.
- When several outputs show the same scene package, only one plays its sound: the first active
  one by output name whose scene volume is above 0, which hands the sound over to the next one
  when it pauses or is covered.
- Sound layers start, stop and change their volume and pan as their values change on the scene
  clock. Values that change over time are read at the frame rate of the output, without rendering
  a frame while nothing visible changes; fades and volume changes are applied to every sample, and
  values that stay the same cost nothing.
- `GetState().outputs[x].sound` is `""` for content without sound layers, `"off"` while the sound
  is muted, has not started or every layer has stopped, `"playing"`, `"paused"`, or `"elsewhere"`
  when another output plays it.

## Change notification

`Serial` starts at the daemon's start time in microseconds since the Unix epoch and grows by one
for every `Changed` signal. Topics and their `ids`:

| Topic | Emitted when | `ids` |
|-------|--------------|-------|
| `state` | Anything in `GetState` changes | output names, or `[]` for global changes |
| `library` | Items, sources or tags change | item uids, or `[]` for many items, sources or tags |
| `playlists` | Playlists change, start or stop | playlist uids |
| `profiles` | Profiles or presets change or are activated | profile or preset uids |
| `providers` | Providers change | provider ids |
| `repos` | Repositories or installed packages change | repository ids, or `[]` |
| `jobs` | A job is created or finishes | job ids as decimal strings |
| `inputs` | A grant or a decline changes, an output starts or stops showing a scene that declares inputs, or audio capture or the pointer feed starts or stops | `[]` |
| `account` | The sync server, this device's link or the sync state change | `[]` |

Per topic, the first change emits `Changed` at once and opens a 250 ms window; changes inside the
window are merged and emitted once when it closes. Merged `ids` are the union, or `[]` when one
change had no specific ids or the union exceeds 256 ids, meaning "re-read everything of this
topic".

Every query reply carries the `serial` read before the query ran. A client that has seen a
`Changed` of that reply's topic with a larger `serial` fetches again; otherwise the reply is
current.

`WaitChanged(serial, timeout_ms)` returns at once with `(Serial, topics changed after serial)`
when `serial` is older than `Serial` (all eight topics when it is older than the last 256
changes), and with `(Serial, all topics)` for a `serial` of a previous daemon run or one larger
than `Serial`. When `serial` equals `Serial` it waits until a change or the timeout, then
returns `(Serial, [])` on timeout.

## Jobs

Job ids start at 1 for every daemon run and are never reused in a run; clients notice a restart
of the daemon through `NameOwnerChanged`. Starting a job whose kind and target equal a queued or
running job returns that job's id (`import` jobs are never merged). Library jobs (`scan`,
`import`, `analyze`) run one at a time, repository jobs (`repo_add`, `repo_refresh`, `install`)
run one at a time, at most two `provider_sync` jobs download at once, `account`, `account_sync`
and `upload` jobs run one at a time, and so do `link` jobs; a job waiting for its
turn is `queued`. A sync after its downloads and an install after its commit stay `running`
while they wait for the library.

| Kind | Started by | Target | Progress unit | Output |
|------|------------|--------|---------------|--------|
| `scan` | `AddSource`, `Rescan`, folder changes | source uid, or `""` for all | files (`0/0` while walking) | `ScanOutput` |
| `import` | `Import` | `""` | files | `ImportReport` |
| `analyze` | after a scan or import that added or changed items | `""` | items | `AnalyzeReport` |
| `provider_sync` | `Providers1.Sync`, schedule | provider id, or `""` | items | `SyncOutput` |
| `repo_add` | `Repos1.Add` | repository URL without query | `0/0` | `Repo` |
| `repo_refresh` | `Repos1.Refresh`, schedule | repository id, or `""` | `0/0` | `RefreshOutput` |
| `install` | `Repos1.Install` | `repo_id/id@version` | bytes | `InstalledPackage` |
| `account` | `Sync1.AddServer`, `PreviewCode`, `Approve`, `ConfirmShare`, `Rotate`, `Reset`, `SetEscrow` | server URL, `preview <code>`, `approve <code>`, `share <device id>`, `rotate`, `reset` or `escrow` | `0/0` | server answer, or `{recovery}` |
| `link` | `Sync1.Link`, `Join`, `ShareKey`, `Restore` | `link`, `join <device id>`, `share <device>` or `restore` | `0/0` | `{device, encrypted, needs_key}`, `{from}`, `{device, code}` or `{epoch}` |
| `account_sync` | `Sync1.SyncNow`, schedule | `""` | `0/0` | `{applied, records, quarantined}` |
| `upload` | `Sync1.Upload`, `ShareFiles`, `FetchFile`, after a sync with private files | random id, `files` or `fetch` | bytes, or segments for `fetch` | `UploadOutput`, `{uploaded}` or `{item}` |

`CancelJob` drops a queued job; a running job stops at its next checkpoint (between scan
batches of at most 256 files, imported files, analyzed items, provider downloads and download
chunks). Finished steps stay: a cancelled scan keeps the batches it stored and marks nothing
missing, and a package whose commit finished stays installed. Cancelling a finished job changes
nothing.

## Thumbnails

Thumbnails are JPEG files (quality 85) whose longest edge is 256, 512 or 1024 pixels, never
upscaled, in `~/.cache/wallpaper-rs/thumbs/<size>/<key>.jpg`, where `key` is the lowercase hex
BLAKE3 hash of `"<item path>\0<mtime>\0<size>"`; a changed file gets a new key. Directories are
`0700` and files `0600`. Clients only get paths from the daemon and open them read-only.

Only an isolated worker process (`wprsd --worker`, at the lowest CPU and I/O priority) decodes
files: images with size limits, EXIF orientation applied and the first frame of animations;
videos at 10 % of their duration (between 1 s and 30 s); packages from their preview, else from
their image or video entry. The same run measures the item (dimensions, duration, audio,
dominant color) or records why it cannot be decoded. At most `max(1, min(4, cores / 2))` files
are decoded in parallel, each within 15 s, and a worker exits after 30 s without work. Requests
waiting in `Thumbnail` come first, then `RequestThumbnails`, then background analysis.

Every returned path has its modification time refreshed. When the cache grows beyond
`library.thumb_cache_mb`, the oldest files are deleted until it is below 90 % of the limit.
Removing items deletes their thumbnails of every size.

## JSON model

Every reply below carries `model` (currently `1`) and `serial` first. Every field is always
written (`null` for none); clients ignore fields they do not know, and read enum values they do
not know as unknown. Adding a field, an enum value or a method keeps the model version;
removing, renaming or retyping something increases it. Times are Unix seconds; `Fit` is
`"fill"`, `"fit"`, `"stretch"`, `"center"`, `"tile"` or `"span"`; enums are written in snake case.
`edited` tells whether an edit applies to the shown image and `screen` repeats `SetOutputInfo`.
`parallax` tells whether the image moves in depth and `frames` counts the frames rendered for the
output since it appeared. `refresh` is the refresh rate of its screen in Hz (`0` while unknown)
and `fps` the most frames per second it renders now (see "Frame rate"). `buffers` is `"dmabuf"`
or `"memfd"` while a consumer streams the output, and `""` otherwise (see "Stream buffers").
`sound` is `""`, `"off"`, `"playing"`, `"paused"` or `"elsewhere"` (see "Scene sound").

```rust
struct State {
    model: u32, version: String, adapter: String, config: PathBuf, config_error: Option<String>,
    library_error: Option<String>, serial: u64, paused: bool, on_battery: bool, locked: bool,
    volume: f64, muted: bool, profile: Option<Uid>, jobs: u32,
    options: BTreeMap<String, Value>,
    outputs: BTreeMap<String, OutputState>,
}
struct OutputState {
    width: u32, height: u32, image: Option<PathBuf>, fit: Fit, background: String,
    kind: Shown, playing: bool, active: bool, covered: bool, clients: u32, error: Option<String>,
    item: Option<Uid>, preset: Option<Uid>, playlist: Option<OutputPlaylist>, credit: Option<Credit>,
    edited: bool, screen: Option<Screen>, parallax: bool, frames: u64, refresh: u32, fps: u32,
    buffers: String, sound: String, shown_fit: Fit,
}
struct Screen { x: i32, y: i32, width: u32, height: u32, scale: f64, px_per_mm: f64, identity: String }
enum Shown { Color, Image, Video, Scene }
struct OutputPlaylist { uid: Uid, name: String, position: u32, count: u32, next_at: Option<Time> }
struct SceneProperties {
    model: u32, serial: u64, output: String, package: String, title: String,
    properties: BTreeMap<String, Value>, values: BTreeMap<String, Value>, presets: Vec<Value>,
    fields: Vec<SceneField>, preset_labels: Vec<SceneOption>, warnings: Vec<String>,
    user_presets: Vec<SceneOption>, shared: bool, kind: String,
}
struct SceneField {
    key: String, kind: String, label: String, help: String, value: Value, visible: bool,
    min: Option<f64>, max: Option<f64>, step: Option<f64>, integer: bool, unit: String,
    options: Vec<SceneOption>, alpha: bool, multiline: bool, max_len: Option<u32>, accept: Vec<String>,
    enabled: bool, group: String, icon: String, decimals: Option<u32>, editable: bool, log: bool,
    collapsed: bool, secret: bool, filled: bool,
}
struct SceneOption { value: Value, label: String, icon: String }

struct Schema {
    model: u32, serial: u64, options: Vec<Field>, rules: Vec<Field>, auto: Vec<Field>,
    preset: Vec<Field>, providers: Vec<ProviderKind>, query: Vec<QueryKey>, thumb_sizes: Vec<u32>,
    fits: Vec<Fit>,
}
struct Field {
    key: String, kind: FieldKind, default: Value, min: Option<f64>, max: Option<f64>,
    step: Option<f64>, values: Vec<String>, required: bool, clearable: bool,
}
enum FieldKind { Bool, Int, Float, Text, Enum, Color, Interval, Times, Weekdays, List, Url }
struct ProviderKind { kind: String, hosts: Vec<String>, options: Vec<Field>, secret: Option<SecretField> }
struct SecretField { name: String, required: bool }
struct QueryKey { key: String, hint: String }

struct Job {
    id: u64, kind: JobKind, target: String, state: JobState, done: u64, total: u64,
    started_at: Time, finished_at: Option<Time>, error: Option<JobError>, output: Option<Value>,
}
enum JobKind { Scan, Import, Analyze, ProviderSync, RepoAdd, RepoRefresh, Install, Account, Link, AccountSync, Upload }
enum JobState { Queued, Running, Done, Failed, Cancelled }
struct JobError { name: String, message: String }
struct JobsReply { model: u32, serial: u64, jobs: Vec<Job> }
struct JobReply { model: u32, serial: u64, job: Job }

enum ItemKind { Image, Video, Package }
struct PackageInfo {
    id: String, version: String, kind: String, license: String, content: Option<String>,
    photosensitive: Option<bool>, facets: Vec<String>, confirmed: Vec<String>,
}
struct About {
    schema: String, title: String, description: String, changes: String, authors: Vec<String>,
    license: String, content: Option<String>, descriptors: Vec<String>, photosensitive: Option<bool>,
    origin: Option<String>, derived_from: Vec<Work>, credits: Vec<PackageCredit>, links: Vec<Link>,
    inputs: Vec<String>, features: Vec<Feature>, languages: Vec<String>, provider: Option<String>,
    source_url: Option<String>, platforms: Vec<String>, packages: Vec<String>,
}
struct Work { author: String, title: Option<String>, url: Option<String>, license: Option<String> }
struct PackageCredit {
    paths: Vec<String>, license: String, attribution: String, url: Option<String>, source: Option<String>,
}
struct Link { kind: String, url: String }
struct Feature { name: String, why: String }
struct Item {
    uid: Uid, source: Option<Uid>, kind: ItemKind, path: PathBuf, title: String, size: u64,
    mtime: Time, width: Option<u32>, height: Option<u32>, duration_ms: Option<u32>,
    audio: Option<bool>, color: Option<u32>, error: Option<String>, package: Option<PackageInfo>,
    tags: Vec<String>, rating: u8, favorite: bool, added_at: Time, used_at: Option<Time>,
    use_count: u32, missing: bool,
}
struct ItemSummary {
    uid: Uid, kind: ItemKind, title: String, width: Option<u32>, height: Option<u32>,
    duration_ms: Option<u32>, color: Option<u32>, tags: Vec<String>, rating: u8, favorite: bool,
    missing: bool, error: Option<String>, revoked: Option<Reason>, removable: bool, imported: bool,
    thumb: Option<PathBuf>, package: Option<PackageInfo>,
}
struct SearchPage { model: u32, serial: u64, query: String, total: u32, offset: u32, items: Vec<ItemSummary> }
struct ItemDetails {
    model: u32, serial: u64, item: Item, thumb: Option<PathBuf>, presets: Vec<Preset>,
    playlists: Vec<PlaylistRef>, credit: Option<Credit>, installed: Option<InstalledPackage>,
    revoked: Option<Reason>, about: Option<About>,
}
struct PlaylistRef { uid: Uid, name: String }
struct Source {
    uid: Uid, path: PathBuf, recursive: bool, scanned_at: Option<Time>, error: Option<String>,
    items: u32, provider: Option<String>,
}
struct SourcesReply { model: u32, serial: u64, sources: Vec<Source> }
struct Tag { name: String, count: u32 }
struct TagsReply { model: u32, serial: u64, tags: Vec<Tag> }
struct SourceScan { source: Option<Uid>, added: u32, updated: u32, moved: u32, missing: u32 }
struct ScanOutput { reports: Vec<SourceScan> }
struct Imported { path: PathBuf, uid: Uid }
struct PathError { path: PathBuf, error: String }
struct ImportReport { imported: Vec<Imported>, failed: Vec<PathError> }
struct AnalyzeReport { analyzed: u32, failed: u32 }
struct Credit { provider: String, title: String, attribution: Option<String>, license: Option<String>, link: Option<String> }

enum Order { Sequential, Shuffle }
struct Rules {
    order: Order, interval: String, at: Vec<String>, days: Vec<String>, synced: bool,
    resume: bool, advance_while_paused: bool, skip_videos_on_battery: bool,
    transition_ms: Option<u32>,
}
struct Playlist { uid: Uid, name: String, query: Option<String>, rules: Rules, count: u32, outputs: Vec<String> }
struct PlaylistsReply { model: u32, serial: u64, playlists: Vec<Playlist> }
struct PlaylistPage { model: u32, serial: u64, playlist: Playlist, total: u32, offset: u32, items: Vec<ItemSummary> }
struct HistoryEntry { at: Time, item: Option<ItemSummary> }
struct HistoryReply { model: u32, serial: u64, output: String, entries: Vec<HistoryEntry> }
struct ProfileOutput {
    item: Option<Uid>, path: Option<PathBuf>, playlist: Option<Uid>, fit: Option<Fit>,
    background: Option<String>, preset: Option<Uid>,
}
struct ProfileAuto { outputs: Option<Vec<String>>, on_battery: Option<bool> }
struct Profile {
    uid: Uid, name: String, outputs: BTreeMap<String, ProfileOutput>,
    options: BTreeMap<String, String>, auto: ProfileAuto, priority: i32, active: bool,
}
struct ProfilesReply { model: u32, serial: u64, active: Option<Uid>, profiles: Vec<Profile> }
struct ProfileReply { model: u32, serial: u64, profile: Profile }
struct PresetValue { fit: Option<Fit>, background: Option<String> }
struct Preset { uid: Uid, item: Option<Uid>, name: String, value: PresetValue }
struct PresetsReply { model: u32, serial: u64, item: Option<Uid>, presets: Vec<Preset> }

enum ProviderState { Idle, Syncing, Locked, Error, NeedsSecret }
struct Provider {
    id: String, kind: String, options: BTreeMap<String, String>, source: Option<Uid>,
    state: ProviderState, has_secret: bool, hosts: Vec<String>, items: u32, cache_bytes: u64,
    last_sync: Option<Time>, next_sync: Option<Time>, last_error: Option<String>, job: Option<u64>,
}
struct ProvidersReply { model: u32, serial: u64, providers: Vec<Provider> }
struct SyncReport {
    provider: String, listed: u32, downloaded: u32, reused: u32, skipped: u32, failed: u32,
    scan: SourceScan,
}
struct SyncOutput { reports: Vec<SyncReport> }
enum RepoWarning { RootExpiring, RootExpired, EntryExpiring, Stale }
struct Repo {
    repo_id: String, url: String, name: String, hosts: Vec<String>, fingerprints: Vec<String>,
    root_version: u64, root_expires: Time, entry_version: u64, entry_expires: Time, checked: Time,
    stale: bool, packages: u32, warnings: Vec<RepoWarning>, last_error: Option<String>,
    job: Option<u64>,
}
struct ReposReply { model: u32, serial: u64, repos: Vec<Repo> }
enum Reason { Malware, License, Takedown, Broken, Other }
struct RepoPackage {
    repo_id: String, repo_name: String, id: String, author: Option<String>, version: String,
    kind: String, title: String, description: String, tags: Vec<String>, license: String,
    authors: Vec<String>, size: u64, published: Time, preview: bool, versions: Vec<String>,
    installed: Vec<String>, update: bool, content: Option<String>, photosensitive: Option<bool>,
    facets: Vec<String>, changes: String,
}
struct BrowsePage { model: u32, serial: u64, query: String, total: u32, offset: u32, packages: Vec<RepoPackage> }
struct InstalledPackage {
    repo_id: String, id: String, version: String, sha256: String, size: u64,
    signer: Option<String>, revoked: Option<Reason>, item: Option<Uid>, update: Option<String>,
    changes: String,
}
struct InstalledReply { model: u32, serial: u64, packages: Vec<InstalledPackage> }
struct Update { repo_id: String, id: String, installed: String, available: String, changes: String }
struct RefreshOutput { repos: Vec<Repo>, updates: Vec<Update>, revoked: Vec<InstalledPackage> }

enum Input { Audio, Pointer }
struct SceneInputs { scene: String, audio: bool, pointer: bool, title: String }
struct InputsReply {
    model: u32, serial: u64, capturing: bool, tracking: bool,
    grants: BTreeMap<String, Vec<Input>>, shown: BTreeMap<String, SceneInputs>, parallax: Vec<String>,
    declined: BTreeMap<String, Vec<Input>>,
}
enum AccountState { Unlinked, Linking, NeedsKey, Ready, Paused, Error }
struct AccountDevice {
    id: String, name: String, label: Option<String>, platform: String, created_at: Time,
    seen_at: Option<Time>, current: bool,
}
enum Recovery { Off, Current, Stale }
struct RepoOffer { id: String, repo_id: String, name: String, url: String, fingerprints: Vec<String> }
struct Pairing { device: String, code: Option<String>, sharing: bool }
struct Account {
    url: String, name: String, repo_id: Option<String>, handle: Option<String>,
    device: Option<String>, label: String, state: AccountState, encrypted: Option<bool>,
    warning: Option<String>, paused: bool, last_sync: Option<Time>, last_error: Option<String>,
    records: u32, used_bytes: u64, quota_bytes: u64, devices: Vec<AccountDevice>,
    pairing: Option<Pairing>, quarantined: u32, unapplied: u32, offers: Vec<RepoOffer>,
    secrets: Vec<String>, recovery: Recovery, files_bytes: u64, files_quota_bytes: u64,
    job: Option<u64>,
}
struct AccountReply { model: u32, serial: u64, account: Option<Account> }
struct PrivateFile { id: String, name: String, size: u64, item: Option<Uid>, shared: bool, stored: bool }
struct FilesReply { model: u32, serial: u64, folders: Vec<String>, files: Vec<PrivateFile> }
struct UploadOutput { id: String, state: String, package: Option<String>, version: Option<String>, errors: Vec<String> }
```

Bulk replies (`Search`, playlist pages, history, installed packages) carry no file paths except
cached thumbnail paths; an item's path appears only in `GetItem` and in `GetState().outputs`.

## Configuration file

`~/.config/wallpaper-rs/config.toml` is written atomically with mode `0600`; the directory is
created with mode `0700`. If the file is a symbolic link, the link is kept and its target is
replaced. Values out of range are clamped when the file is read and invalid colors fall back to
the default color, with a warning naming the key.

If the file cannot be parsed at startup, the daemon starts with the default configuration and
reports the error in `GetState().config_error` (`null` when the file is fine), as
`invalid configuration at line L, column C: <message>`. Before the first save, the broken file
is copied to `config.toml.broken` (mode `0600`).

```toml
profile = "01J…"          # active profile uid, absent when none

[default]                 # and each [outputs.<name>]
image = "/path"           # may be a .wpak written by Library1.Apply
fit = "fill"
background = "#000000"
playlist = "01J…"         # playlist assigned to this section
preset = "01J…"           # label of the preset applied to this section

[default.scenes."org.example.calm"]   # and [outputs.<name>.scenes."<package id>"]
tint = "#ff8800"          # values of the settings of a scene or video package

[library]
watch = true
thumb_cache_mb = 512

[repos]
auto_refresh = true

[models]
allow_download = true

[providers.photos]        # one table per provider
kind = "immich"
server = "https://photos.example.org"
album = "0d6a…"
allow_plain_http = false
refresh = "1h"
cache_mb = 1024

[layout]                  # only written when set
bezel_mm = 8.0            # frame width of every screen side, for the span fit

[layout.monitors."Maker Model 1234"]
mm = [0.0, 0.0, 596.7, 335.6]
```

A section shows its `playlist` if set, else its `image`. An output section that sets either one
overrides the content of `[default]` as a whole; `fit` and `background` still fall back field by
field. `preset` is only a label: `Library1.Apply`, `Profiles1.Activate` and
`Profiles1.SavePreset` write the effective fit and background into the section. When the
library is available at startup or `Reload`, `preset`, `playlist` and `profile` uids that name
nothing are dropped with a warning naming the key, and the file is saved.

Each `[providers.<id>]` table holds the provider's `kind` and the options given to it; missing
options take the defaults of `GetSchema().providers`. Secrets are never written here. A table
that cannot be read is skipped with a warning naming the provider and kept unchanged when the
daemon saves the file.
