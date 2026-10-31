# Web pages

A `web` package (`package.md`, "Web pages") is shown by `wprs-web`, a helper process that the
daemon starts for each web package shown and stops as soon as no screen shows it. The helper runs
the page in WPE WebKit, hands every frame to the daemon, and the daemon draws it like any other
wallpaper: with the same transitions, fit, frame rate cap, pause rules and streams.

## Engine

`wprs-web` loads WPE WebKit 2.54 or newer (`libWPEWebKit-2.0.so.1`, with its headless platform)
when it starts, so the same binary runs everywhere and nothing links against WebKit at build time.
WPE is the only engine that renders a page without a window and hands over its frames: other
WebKit ports only draw into toolkit widgets or take still snapshots, and engines that are not in
distributions would have to be bundled and updated by wallpaper-rs. Where it is missing the daemon
refuses web wallpapers with a translated reason, in `Library1.Apply`, `wprs-ctl lib apply` and the
error of the screen, instead of drawing nothing:

| Where | What it has | Web pages |
|-------|-------------|-----------|
| Arch Linux, Fedora | `wpewebkit` 2.54 | shown |
| Ubuntu 24.04 (the CI runner) | WebKitGTK 2.52 only, no WPE | refused: `they need WPE WebKit 2.54 or newer, which is not installed` |
| Flatpak (`org.kde.Platform`) | no WebKit in the runtime | refused with the same reason; `wprs-web` is in the bundle |
| any, without the helper | — | refused: `the web helper wprs-web is not installed` |

The daemon finds the helper in `WPRS_WEB`, next to `wprsd` or in `PATH`, and runs
`wprs-web --check` once to learn whether it can start (it prints the WebKit version, or why not).

## Frames

The helper takes over the `render_buffer` and `buffers_changed` steps of the headless view, so it
decides when a frame is done: it reports a frame as presented only at the frame rate the daemon asks
for (the output's cap, battery and power saving included), and WebKit paces animation frames, CSS
animations and video by that. A page that does not change draws nothing.

When the daemon's GPU can import them, the helper asks WebKit to render into the formats and
modifiers that GPU samples in place (`XRGB8888`/`ARGB8888` with one plane, on the render node of the
daemon) and sends each buffer once as a DMA-BUF (`decode` reads `zero-copy`); the daemon gives a
buffer back once a newer frame replaced it. Otherwise frames travel through a sealed `memfd` with two
slots (`decode` reads `copy`). When the page draws nothing on the GPU within 10 s, it starts again
without access to the GPU devices and draws in memory. Pages are always opaque.

## Lifecycle

- A page is visible while its screen is active and seen; a stream that loses its last consumer
  hides it after 2 s, so readers that come and go do not restart it. Hidden pages get
  `document.visibilityState = "hidden"`, stop their animation frames and draw nothing; after 5
  minutes hidden the web process is stopped and the last frame stays on screen until it is visible
  again, when the page loads again.
- The helper reloads a page whose web process crashed, up to three times. The daemon restarts a
  helper that stopped or failed after 1, 5 and 30 s; after the third failure in a row the screen
  shows its background color and its error says why.
- Changing a property sends only the changed values to the running page; changing what the page may
  see (below), clearing its data or granting the network starts it again.

## Protocol

The helper gets one end of a `SOCK_SEQPACKET` socket pair as descriptor 3. Each packet is one JSON
message (`ToWeb` from the daemon, `FromWeb` from the helper, in `apps/wprs-web/src/lib.rs`), at
most 192 KiB, with buffer descriptors passed as `SCM_RIGHTS`. The helper dies with the daemon
(`PR_SET_PDEATHSIG`).

## Sandbox

Every layer applies to every page:

1. The page only reads its own package through `wpak://<package id>/<path>`: only files listed in
   the package (never `manifest.json` or its signature), percent-decoded, without `..`. A file the
   user chose reaches it only through an opaque `wpak-user://<property>/<token>` (below). Nothing
   else resolves: `file:`, other packages, other schemes.
2. A content security policy that the page cannot remove limits every load to `wpak:`,
   `wpak-user:`, `data:` and `blob:`, plus the declared origins once the network is allowed; no
   objects, forms or `<base>`.
3. A content filter blocks every request but those schemes and origins, including `fetch` and
   WebSockets.
4. Navigation outside the package, new windows, downloads, dialogs, the file chooser,
   notifications and full screen are refused; every permission request (camera, microphone,
   location, clipboard and the rest) is denied; WebRTC and media capture are off; keys never reach
   the page.
5. Unless the package declares origins and the user granted `network`, the helper starts in its own
   user and network namespaces with no network at all, so not even the network process of WebKit
   can reach anything.
6. WebKit runs the page in its own process inside its bubblewrap and seccomp sandbox, limited to
   768 MB of memory.

`OutputState.sandbox` is `"on"` when every layer is in place and `"reduced"` when WebKit could not
start its sandbox or the namespaces could not be made (some containers forbid them); clients show
it next to the privacy notice.

## Privacy

A page sees, by default, the real time zone, the language of the user, the model of the graphics
card through WebGL, and keeps its own storage when it declares `web.storage_kib`. Each of them is
restricted per package with `Library1.Confirm` (`hide_zone`: UTC; `hide_language`: `en-US`;
`hide_gpu`: WebGL keeps working with a generic vendor and renderer and no debug extension, and
`navigator.gpu` is absent; `hide_storage`: nothing is kept). `PackageInfo.exposes` lists what a page
sees now, and the app (details and *Apply to…*), the tray (each screen) and `wprs-ctl` (`lib apply`
and `lib show`) show the same notice from it: "This web wallpaper uses your time zone, your
language, the model of your graphics card and data it keeps on this computer."

Storage lives in `~/.local/share/wallpaper-rs/web/<package id>/` (mode `0700`), is never synced,
and goes away with `Library1.ClearData` (`wprs-ctl lib clear-data`, *Clear data* in the app) and
when the last item of the package is removed. The network is the `network` input of `Inputs1`,
granted only from the app, the tray or `wprs-ctl` (`wprs-ctl input grant <id> network`); a page
reaches only the origins it declares.

## The page interface

The helper injects `window.wprs` before the page's own scripts (and before `web.preload`):

| Member | What it does |
|--------|--------------|
| `properties`, `onProperties(cb)` | all values when the callback is added, then only the changed ones; colors as `{hex, r, g, b, a}` (0 to 1), combos as `{value, label}`, files as `wpak-user://` tokens, folders as `wpak-user://<id>/` |
| `fps`, `quality`, `locale`, `volume`, `onSettings(cb)` | the frame rate it is drawn at, the quality level (0 to 3), the language picked among the package's, and the wallpaper volume (0 to 1) for its own sound; the page is muted at 0 |
| `paused`, `onPause(cb)` | `true` while hidden |
| `onAudio(cb)` | `{left, right}`, 64 bands each from bass to treble, at most 30 times a second, once the user allows `audio` for the package; the first callback subscribes |
| pointer and clicks | real DOM events: `pointermove` while `pointer` is allowed, and `click`s only from real clicks on screens where the user turned clicks on and the package holds `clicks` |
| `files(prop)`, `randomFile(prop)`, `onFiles(prop, cb)` | the tokens of a chosen folder: `files` (and the `{added, removed}` notices) only with `mode: all`; `randomFile` in both modes |
| `onAction(cb)` | the id of a pressed `button` property |
| `onMedia`, `openShortcut`, `rgb.setImage` | present and inert: their features are refused until they work |

`console` output and uncaught errors reach the daemon's log at debug level, at most 200 lines a
run. The folder of a `folder` property is listed by the daemon (`max_files`, `recursive`, hidden
files and links left out, the same rules as scenes), checked again every 5 s while the page is
shown and never while hidden; the helper serves only the listed files, opened without following
links.
