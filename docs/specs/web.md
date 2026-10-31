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
| Android (the app) | the system WebView | shown with it, see "On the phone" |

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
- The helper asks each page for an answer every 5 s while its web process runs; a page that does
  not answer for 10 s (a script stuck in a loop) is stopped and loads again like a crashed one. The
  helper reloads a page whose web process crashed, ran out of memory or stopped answering after 1, 4
  and 9 s; a fourth time in a row turns the page off, the screen shows its background color and its
  error says why. A page that runs for a minute without crashing starts counting again. The daemon
  restarts a helper that stopped or failed after 1, 5 and 30 s; after the third failure in a row the
  screen shows its background color and its error says why.
- The helper and WebKit's processes run at nice 10. When a systemd user manager runs, the helper
  starts in its own transient scope with `CPUQuota=100%`, `CPUWeight=20`, `MemoryMax=1536M`,
  `MemorySwapMax=0` and `TasksMax=512`, so a page takes at most one processor and 1.5 GB in all;
  where the scope cannot be made the helper starts without it.
- A page whose kept data (`data` below) grows past twice its `web.storage_kib` plus 1 MiB starts
  without keeping anything, and its error says so until its data is cleared. WPE WebKit has no
  quota per origin in its current interface, so the limit is checked when the page starts.
- Changing a property sends only the changed values to the running page; changing what the page may
  see (below), clearing its data or granting the network starts it again.

## Protocol

The helper gets one end of a `SOCK_SEQPACKET` socket pair as descriptor 3. Each packet is one JSON
message (`ToWeb` from the daemon, `FromWeb` from the helper, in `apps/wprs-web/src/lib.rs`), with
buffer descriptors passed as `SCM_RIGHTS`; a message over 32 KiB (long folder listings) travels in a
sealed `memfd` instead, up to 64 MiB. The daemon maps frames only from `memfd`s sealed against
shrinking and as large as the frames they hold. The helper dies with the daemon
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

A page sees, by default, the real time zone, the language of the user, what the graphics card can
do through WebGL, and keeps its own storage when it declares `web.storage_kib`. On the desktop WPE
WebKit already reports a generic renderer (`Apple GPU`), so WebGL tells the page the limits and
extensions of the card, not its model; on the phone the WebView reports the model. Each of them is
restricted per package with `Library1.Confirm` (`hide_zone`: UTC; `hide_language`: `en-US`, also in
`wprs.locale` and the labels of combos; `hide_gpu`: WebGL keeps working with a generic vendor and
renderer and no debug extension, and `navigator.gpu` is absent; `hide_storage`: nothing is kept). `PackageInfo.exposes` lists what a page
sees now, and the app (details and *Apply to…*), the tray (each screen) and `wprs-ctl` (`lib apply`
and `lib show`) show the same notice from it: "This web wallpaper uses your time zone, your
language, what your graphics card can do and data it keeps on this computer."

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
| `fps`, `quality`, `locale`, `volume`, `onSettings(cb)` | the frame rate it is drawn at, the quality level (0 to 3), the language picked among the package's, and the wallpaper volume (0 to 1) for its own sound: the page may play sound without a gesture, its `<audio>` and `<video>` play at their own volume times the wallpaper's (`volume` reads what the page set) and an `AudioContext` sends its `destination` through a gain at the wallpaper's volume; the page is muted at 0 |
| `paused`, `onPause(cb)` | `true` while hidden |
| `onAudio(cb)` | `{left, right}`, 64 bands each from bass to treble, at most 30 times a second, once the user allows `audio` for the package; the first callback subscribes |
| pointer and clicks | real DOM events: `pointermove` while `pointer` is allowed, and `click`s only from real clicks on screens where the user turned clicks on and the package holds `clicks` |
| `files(prop)`, `randomFile(prop)`, `onFiles(prop, cb)` | the tokens of a chosen folder: `files` (and the `{prop, added, removed, changed}` notices, `changed` as `[old, new]` pairs for files rewritten in place) only with `mode: all`; `randomFile` in both modes; a token changes with the size and time of its file, so a rewritten file loads anew |
| `onAction(cb)` | the id of a pressed `button` property |
| `onMedia`, `openShortcut`, `rgb.setImage` | present and inert: their features are refused until they work |

Audio and video files of the package and of the user's folder play from a `blob:` copy that the
helper's script makes when a media element (or `new Audio`) gets a `wpak:` or `wpak-user:` address,
because GStreamer only reads the schemes it knows; the copy lives while the element uses it, and a
file over 256 MiB does not play. `console` output and uncaught errors reach the daemon's log at
debug level, at most 200 lines a run. The folder of a `folder` property is listed by the daemon (`max_files`, `recursive`, hidden
files and links left out, the same rules as scenes), checked again every 5 s while the page is
shown and never while hidden; the helper serves only the listed files, opened without following
links.

## On the phone

The Android app shows the same packages with the system WebView, the engine Android keeps up to
date for every app, instead of `wprs-web`: packages never bring an engine, and the app starts no
process of its own. The package, `window.wprs`, the privacy settings and the notice are the same;
these are the differences:

- **Drawing.** The page is laid out at the size of the wallpaper on a private virtual display whose
  output is the wallpaper's own surface, so its frames reach the screen without copies. The change
  to or from a page has no fade.
- **Addresses.** The page lives at `https://<16 hex>.wpak.invalid/`, a host made from the package id
  that never resolves, and every request goes through the app: it serves only the files the package
  lists (never `manifest.json`), the interface at `/:wprs/api.js` (added to every HTML file, after
  its doctype) and the files the user chose as `https://<host>/:user/<property>/<token>` instead of
  `wpak-user://`. Both are opaque to the page. The content security policy is the one above with
  `'self'` in place of the schemes of the desktop.
- **Frame rate.** `requestAnimationFrame` is paced at the cap by the injected script; CSS animations
  and videos run at the rate of the display.
- **Lifecycle.** A page is visible while its screen is shown and the phone neither saves power, nor
  runs low on battery, nor is very hot; otherwise the WebView is paused (`visibilityState` is
  `"hidden"` and `onPause(true)` fires). After 5 minutes hidden it is closed and loads again when
  shown. A renderer that crashes loads the page again, up to three times. Touches reach the page
  only after its first paint.
- **Sandbox.** Layers 1 to 4 are the same, enforced by intercepting every request. Network loads
  are blocked in the WebView unless the package declares origins and the user allowed `network`,
  and only those origins pass; cookies, service workers, safe browsing and the WebView's metrics
  are off, and peer connections (WebRTC) are removed from the page and from every frame of it (see
  the guard below). The renderer runs in the WebView's isolated process.
- **Privacy.** `hide_zone` and `hide_language` are applied by a guard script that runs before the
  page's: `Date`, `Temporal.Now`, `Intl`, the `toLocale…` methods, `localeCompare` and
  `navigator.language` see UTC and `en-US`, also through `constructor`. The WebView runs the guard
  at the start of every frame of the page's origin (`addDocumentStartJavaScript`), and the guard
  also applies itself to the frames the page creates, reads through `contentWindow` or reaches
  through `window[n]`, to its workers (each starts from `/:wprs/worker.js`, which runs the guard
  and then the page's script) and to its worklets. `hide_gpu` uses the same mask as the desktop,
  where the phone's WebView would report the model of the graphics chip. Kept storage is the
  WebView's storage of the page's origin; *Clear data* and removing the last item of the package
  move the page to a new origin and delete the old one, and a page that keeps nothing gets a new
  origin each time it loads, deleted when it closes. The notice is shown on the page of the
  package, above *Set as wallpaper*, in its *Privacy* section with one switch per item and *Clear
  data*, and as a message after importing it.
- **Inputs.** *React to touch* is the pointer (`pointermove` from the finger, as a mouse hover) and
  *React to taps* the clicks: a tap that does not move becomes one real click. With both the page
  gets the touches as they are. The lock screen gives none.
- **Sound.** The page's own sound follows the phone's sound setting for videos: `wprs.volume` is 0,
  and the page muted, unless video sound is on. `onAudio` reads the Visualizer of Android, as scenes
  do, at most 30 times a second.
- **Console.** `console` output and errors reach logcat (tag `wprs`, `page:`), at most 200 lines a
  load.
