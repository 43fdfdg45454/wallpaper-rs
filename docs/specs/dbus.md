# D-Bus interface

Every component takes these names from `wprs_core::dbus`. The command line client (`wprs-ctl`)
and the desktop app are plain clients of this interface: every action is available in both. The
system tray and the desktop integration offer a subset.

| Item | Value |
|------|-------|
| Bus | session |
| Well-known name | `io.github.wallpaper_rs.Daemon` |
| Object path | `/io/github/wallpaper_rs/Daemon` |
| Interfaces | `io.github.wallpaper_rs.Daemon1`, `Library1`, `Playlists1`, `Profiles1`, `Providers1`, `Repos1`, `Inputs1`, all on that object |

The daemon is D-Bus activatable (`packaging/dbus`) and runs as the systemd user unit
`wprsd.service`. In the Flatpak (`packaging/flatpak`) it owns the name through `--own-name`, so
Flatpak exports the same activation file, starts the daemon with `flatpak run` and drops
`SystemdService`; there is no systemd unit there. The name is owned without allowing
replacement. A second daemon checks the name before initializing the GPU and PipeWire and exits
at once with an error saying that another wprsd owns it.

A panic in any thread of the daemon, a lost GPU device or a GPU that stops responding makes the
daemon exit with status 70; the user unit restarts it (`Restart=on-failure`, at most 5 starts
in 60 s) and clients get their streams again through `RequestStream`. Only the methods that
change what is shown (`Library1.Apply`, `Playlists1.Start`, `Stop`, `Next` and `Previous`,
`Profiles1.Activate` and the `Daemon1` methods that change something) need the renderer: while
it is not running they return `Failed` (`the renderer stopped after an error; restart wprsd`)
and change nothing. Every other method keeps working.

The daemon opens network connections only to the providers and repositories the user added
(including the signed mirrors of a repository, the only hosts a repository request may be
redirected to), with a generic `User-Agent:
wallpaper-rs/<version>` and without cookies or `Referer`. At startup it contacts only providers
due for their automatic sync; the daily repository refresh starts 10 minutes later.

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
- Call `ReleaseStream` once for every successful `RequestStream`.

Stream format: raw video, `BGRx` (or `BGRA`), fixed size, variable frame rate, one memfd per
buffer mapped from offset 0. Frames are produced when the picture changes (new wallpaper,
transition, video frame, a scene that animates or whose inputs changed), when a consumer starts
streaming, and every 10 s as a keep-alive.
While no consumer is streaming an output, its video is paused and the daemon renders only the
final picture after each change, which a consumer receives as soon as it connects.

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
has no stream.

### `PreviewEdit(s path, s edit_json)`

Shows the image `path` with `edit_json`, an edit as described in "Edits and spanning", instead
of its saved edit wherever `path` is shown, without saving anything. Calls replace the preview
and are rendered at most once per frame; an empty `edit_json` ends it, and so do 60 s without a
call. `path` is absolute; a relative path, an invalid edit (the message names the field) or more
than 2 MiB of JSON return `InvalidArgs`, a missing file `NotFound`.

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

A `SceneProperties` document for the scene package shown on `output`, or for `output = ""` the
one set in the `[default]` section: its user properties as declared in its manifest, their
current values and its presets. `NotFound` when no scene package is shown there.

### `SetSceneProperty(s output, s id, s value_json)`

Sets one user property of the scene shown on `output` (`""` = the `[default]` section, used by
every output without its own value) to `value_json`, a JSON value of the property's type: a
`#rrggbb` (or `#rrggbbaa` with `alpha`) string for `color`, a number for `slider`, `true` or
`false` for `bool`, one of the options for `combo`, a string for `text` and an absolute path or
`null` for `file`. Numbers out of range are clamped and integers rounded; anything else that does
not fit, an unknown `id` and a file that cannot be read as an image return `InvalidArgs`. Values
are stored per output and per package id in the configuration file (see "Configuration file"),
never inside the package, and apply at once without reloading the scene.

### `ApplyScenePreset(s output, s preset)`

Stores every value of the preset `preset` of the scene shown on `output`, as `SetSceneProperty`
would. `NotFound` for a preset the package does not declare.

### `ResetSceneProperties(s output)`

Forgets the values stored for the scene shown on `output` (`""` = the `[default]` section), so its
properties take their defaults or the `[default]` values again.

### `SetOption(s key, s value)`

Changes one setting and saves the configuration. Keys and their current values are listed in
`GetState().options`, and described in `GetSchema().options`:

| Key | Values | Default |
|-----|--------|---------|
| `playback.volume` | `0.0` to `1.0` | `0.5` |
| `playback.muted` | `true`, `false` | `true` |
| `playback.max_fps` | `1` to `240` | `30` |
| `transition.kind` | `fade`, `none` | `fade` |
| `transition.duration_ms` | `0` to `10000` | `600` |
| `pause.when_covered` | pause while a maximized or fullscreen window covers the output | `true` |
| `pause.on_battery` | pause while running on battery | `true` |
| `pause.when_locked` | pause while the screen is locked | `true` |
| `library.watch` | rescan folder sources when files in them change, one scan per source at most every 5 s | `true` |
| `library.thumb_cache_mb` | `64` to `8192`, size of the thumbnail cache | `512` |
| `repos.auto_refresh` | refresh each repository 10 min after start and then every 24 h | `true` |
| `layout.bezel_mm` | `0` to `100`, frame width of every screen side for the `span` fit | `0.0` |

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
labels: clients translate keys with their own strings and fall back to the key. It never changes
while the daemon runs.

### `SetPaused(b paused)`

Pauses or resumes every animated wallpaper.

### `SetVolume(d volume)`

Sets the volume of video wallpapers, `0.0` to `1.0`.

### `SetMuted(b muted)`

Mutes or unmutes video wallpapers.

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

Volume of video wallpapers. `PropertiesChanged` is emitted whenever it changes, whichever
method changed it (including `SetOption` and `Reload`).

### Property `Muted` (`b`)

Video wallpapers are muted. `PropertiesChanged` is emitted whenever it changes.

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
allowed.

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
(`use Repos1.Uninstall`), belongs to a source while its file still exists (`remove the file or
the folder`: the next scan would add it back), or lies outside
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
thumbnails.

### `GetItem(s uid) → (s json)`

An `ItemDetails`: the item with its path, the cached 512 px thumbnail, its presets, the manual
playlists that contain it, and its provider credit, installed package record and revocation
reason when they apply.

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
sets the item's `error`. A scene package without preview returns `Failed` (`no preview`).

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
missing, has an `error` (invalid packages), or is a revoked package. The item's path,
the effective fit and background and the preset uid (as a label) are written to the affected
configuration sections, and the use is recorded once per affected connected output. Image and
video packages show their entry: it is copied once to `~/.cache/wallpaper-rs/blobs/`. Scene
packages are rendered live (see "Scenes"); the videos they contain are copied there once.

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
- When the wall clock is set or `org.freedesktop.timedate1` reports a new `Timezone`, the next
  `at` time is computed again at once; an interval keeps its next switch unless the clock moved
  back past it, and an overdue switch happens once.
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
- Missing items, items that cannot be decoded and revoked packages are left out, and so are
  videos on battery with `skip_videos_on_battery`. If a playlist has nothing to show, the output
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
changes. The choice also survives a restart of the daemon: the profile chosen with `Activate`
(or cleared with `""`) and the connected outputs and power source at that moment are kept in
`~/.local/share/wallpaper-rs/profile-hold.json` (`0600`) until the outputs or the power source
differ from them. A moment without any output, as while the daemon starts, does not end the
hold.

### `List() → (s json)`

A `ProfilesReply`.

### `Get(s profile) → (s json)`

A `ProfileReply`.

### `Save(s name, b with_options) → (s uid)`

Saves the current assignment of every output, and with `with_options` every setting, as a
profile. An existing profile of that name is replaced and keeps its uid, rules and priority.

### `Activate(s profile)`

Applies a profile at once and marks it active; `""` only clears the active marker. Outputs not
in the profile lose their own wallpaper. Entries whose item, file or playlist is gone or cannot
be shown are skipped with a warning, and those outputs show the default section; unknown or
invalid settings are skipped too. The configuration is saved once and every output is
reconfigured.

### `Rename(s profile, s name)`

Renames a profile.

### `Delete(s profile)`

Deletes a profile; deleting the active profile clears the marker.

### `SetAuto(s profile, s key, s value)`

Sets a rule for automatic activation: `outputs` (comma-separated connector names, met when
exactly these outputs are connected; `""` unsets), `on_battery` (`true`, `false`; `""` unsets)
or `priority` (`-1000` to `1000`).

### `ListPresets(s item) → (s json)`

A `PresetsReply`: the presets of `item`, or the global presets for `item = ""`.

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

A `BrowsePage` of the packages offered, matched without case on id, title, description, tags
and authors, sorted by id then repository. `limit` is `0..=500`.

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
the newest installed version when a newer one is available.

### Signal `PreviewReady(s spec, s path)`

A preview queued by `RequestPreviews` is ready, or could not be made (`path = ""`: unknown
package, no preview, or a failed download or check).

## `io.github.wallpaper_rs.Inputs1`

Scenes may follow the sound that plays and the pointer. Both inputs are off for every scene
until the user grants them to that scene; grants are kept per scene package id in
`~/.local/share/wallpaper-rs/inputs.json` (mode `0600`) and can be revoked at any time.

An input is on only while an output shows a scene that declares it, the screen is unlocked
(whatever `pause.when_locked` says), that output is active (not paused, and not covered while
`pause.when_covered` holds) and the scene holds the grant; the pointer also stops while the
output is covered. Revoking, locking, pausing or hiding the scene stops it at once.

- Audio: the daemon captures the monitor of the default output device through a PipeWire
  stream named "wallpaper-rs audio analysis" (desktop microphone indicators may show it) and
  turns it into at most 64 spectrum bands per channel, levels and beats on the render thread.
  Samples stay in memory only for that analysis: they are never stored, logged or sent anywhere,
  and scenes and scripts only get the bands and levels. A scene without the grant gets silence
  even while another output's scene is listening. The stream is destroyed when no scene needs it.
- Pointer: on layer-shell outputs only the background of an output whose scene uses the pointer
  accepts pointer events, showing the default cursor, so other backgrounds never take desktop
  clicks; in Plasma the daemon loads a KWin script that reports
  `workspace.cursorPos` with `SetPointer`, at most 60 times per second and only while the
  pointer moves, and unloads it when no scene needs it. Keys are never read, and positions are
  neither kept nor logged: each output only has the latest one.

### `Grant(s scene, s input)`

Lets the scene with package id `scene` use `input`, `audio` or `pointer`, whenever it is shown.
`InvalidArgs` for another input or a malformed id (1 to 255 characters of `a-z`, `0-9`, `.`,
`_`, `-`). Granting twice changes nothing.

### `Revoke(s scene, s input)`

Takes `input` back from `scene` and stops it at once if that scene was using it. Revoking an
input that was not granted changes nothing.

### `GetInputs() → (s json)`

An `InputsReply`: the grants, the scenes that declare inputs on each output, whether audio is
being captured (`capturing`) and whether the pointer is being followed (`tracking`).

### `SetPointer(i x, i y)`

The pointer position in logical desktop coordinates, as sent by the KWin script. It is placed
on the output whose `SetOutputInfo` geometry contains it when that output follows the pointer, and
ignored otherwise.

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
  "span": { "center": [0.5, 0.46], "zoom": 1.0 }
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
  (`center`, `radius` `[rx, ry]`, `feather`), `luminance` (`range` `[low, high]`, `soft`) and up
  to 4 `brush` masks (`strokes` of `radius`, `hardness`, `erase` and `points`, at most 20 000
  points in all). Every mask takes `invert`.
- `outputs` crops the edited image for one screen, keyed by its identity (as in
  `SetOutputInfo`) or output name; `connector` names the output as a hint.
- `span` places the image when the fit is `span`: `center` `[x, y]` and `zoom` from `1` to `8`.

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
  level and the GPU class: software renderers allow 24 FPS (15 on battery), integrated GPUs 60
  (30 on battery), dedicated GPUs 144 (60 on battery).
- The quality level follows the GPU class and the power source: on battery or on a software
  renderer heavy effects run at a lower resolution or not at all when the scene marks them
  `optional`, and the scene itself renders at 0.75 or 0.5 of the output size.
- An output that is not active (paused, covered, locked or paused on battery, see
  `SetOption`) or whose stream has no consumer renders nothing and its scene clock stops; it
  continues where it stopped, without catching up.
- `GetState().outputs[x].kind` is `scene` and `playing` tells whether the scene is animating.

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
| `inputs` | A grant changes, or audio capture or the pointer feed starts or stops | `[]` |

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
run one at a time, and at most two `provider_sync` jobs download at once; a job waiting for its
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
    edited: bool, screen: Option<Screen>,
}
struct Screen { x: i32, y: i32, width: u32, height: u32, scale: f64, px_per_mm: f64, identity: String }
enum Shown { Color, Image, Video, Scene }
struct OutputPlaylist { uid: Uid, name: String, position: u32, count: u32, next_at: Option<Time> }
struct SceneProperties {
    model: u32, serial: u64, output: String, package: String, title: String,
    properties: BTreeMap<String, Value>, values: BTreeMap<String, Value>, presets: Vec<Value>,
}

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
enum JobKind { Scan, Import, Analyze, ProviderSync, RepoAdd, RepoRefresh, Install }
enum JobState { Queued, Running, Done, Failed, Cancelled }
struct JobError { name: String, message: String }
struct JobsReply { model: u32, serial: u64, jobs: Vec<Job> }
struct JobReply { model: u32, serial: u64, job: Job }

enum ItemKind { Image, Video, Package }
struct PackageInfo { id: String, version: String, kind: String, license: String }
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
    thumb: Option<PathBuf>,
}
struct SearchPage { model: u32, serial: u64, query: String, total: u32, offset: u32, items: Vec<ItemSummary> }
struct ItemDetails {
    model: u32, serial: u64, item: Item, thumb: Option<PathBuf>, presets: Vec<Preset>,
    playlists: Vec<PlaylistRef>, credit: Option<Credit>, installed: Option<InstalledPackage>,
    revoked: Option<Reason>,
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
    installed: Vec<String>, update: bool,
}
struct BrowsePage { model: u32, serial: u64, query: String, total: u32, offset: u32, packages: Vec<RepoPackage> }
struct InstalledPackage {
    repo_id: String, id: String, version: String, sha256: String, size: u64,
    signer: Option<String>, revoked: Option<Reason>, item: Option<Uid>, update: Option<String>,
}
struct InstalledReply { model: u32, serial: u64, packages: Vec<InstalledPackage> }
struct Update { repo_id: String, id: String, installed: String, available: String }
struct RefreshOutput { repos: Vec<Repo>, updates: Vec<Update>, revoked: Vec<InstalledPackage> }

enum Input { Audio, Pointer }
struct SceneInputs { scene: String, audio: bool, pointer: bool }
struct InputsReply {
    model: u32, serial: u64, capturing: bool, tracking: bool,
    grants: BTreeMap<String, Vec<Input>>, shown: BTreeMap<String, SceneInputs>,
}
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
tint = "#ff8800"          # values of the user properties of a scene package

[library]
watch = true
thumb_cache_mb = 512

[repos]
auto_refresh = true

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
