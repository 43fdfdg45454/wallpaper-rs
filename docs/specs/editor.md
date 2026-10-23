# Scene editor

The scene editor of the app and `wprs-ctl scene` edit a scene folder: `manifest.json`, the
scene file it names as `entry` (`scene.json`) and the assets, by convention under `assets/`. There
is no editor format: the folder is what `wprs-ctl pack build` packs and what the engine reads
(`scene.md`). Both share one implementation, `crates/wprs-editor`, so every action of the app is
also a command, with the same arguments and the same undo history.

## Document

The editor works on one JSON document, `{"manifest": …, "scene": …}`. Every field is addressed by
a JSON Pointer (RFC 6901) into it, such as `/scene/layers/0/opacity`,
`/scene/particles/flakes/init/size` or `/manifest/properties/tint/default`; `~1` stands for `/`
and `~0` for `~` inside a key. The editor keeps its own marks in `scene.ext.editor`, which the
engine ignores: `locked` lists the layers that the canvas does not move, `solo` names the only
layer previews show (its groups and children included; sound layers other than it are muted) and
`hidden` keeps, by layer id, the value of `visible` or `play` that hiding replaced when it was a
binding, keyframes or anything other than `true`.

## Operations

Every change is a list of operations in the style of JSON Patch (RFC 6902):

| Operation | Effect | Inverse |
|-----------|--------|---------|
| `{"op": "add", "path", "value"}` | inserts into a list (an index from 0 to its length, or `-` for the end) or sets a key of an object | `remove`, or `replace` with the old value when the key existed |
| `{"op": "remove", "path"}` | removes an item or a key | `add` with the removed value |
| `{"op": "replace", "path", "value"}` | replaces an existing value | `replace` with the old value |
| `{"op": "move", "from", "path"}` | removes and adds again; a value cannot move into itself | `move` back |

A list of operations applies as a whole: when one fails, the ones before it are undone and the
document stays as it was. Applying a list returns its inverses in reverse order, which undo
applies. List indexes are canonical (`0`, `1`…, never `01`).

## Actions

Actions turn into operations against the current document. The app sends them as
`{"<action>": {<arguments>}}`; `wprs-ctl scene <command> <folder> …` takes the same arguments,
saves at once and prints the ids of what it added. `wprs-ctl scene show DIR [POINTER]` prints the
document or the value at a pointer as JSON, and `wprs-ctl scene history DIR` lists the steps that
undo and redo walk through, the undone ones marked.

| Action | Arguments | Command |
|--------|-----------|---------|
| `set` | `pointer`, `value`; creates the objects on the way | `set DIR POINTER VALUE` |
| `add`, `remove`, `move` | `pointer` and `value`, `pointer`, `from` and `to` | `add`, `remove`, `move` |
| `add_layer` | `kind`, optional `id`, `source` (the asset of an image, video or sound layer, the system of a particles layer), `into` (a group), `at` (0 is the bottom) | `add-layer DIR KIND` |
| `remove_layer`, `duplicate_layer` | `id`; a copy gets new ids for itself and its children | `remove-layer`, `duplicate-layer` |
| `move_layer` | `id`, `to`, optional `into`; without `into` the layer goes to the top level | `move-layer DIR ID TO` |
| `group`, `ungroup` | `ids` of layers with the same parent, each taken once, and an optional group `id`; the `id` of a group, refused while the group has an opacity, transform, anchor, blend, mask, effects, parallax, visibility or clip of its own that ungrouping would lose | `group`, `ungroup` |
| `hide` | `id`, `hidden`; sets `visible`, or `play` on a sound layer, to `false`, keeping what it held under `hidden`, and puts it back when shown | `hide DIR ID --hidden BOOL` |
| `lock`, `solo` | `id` and `locked`; `id`, or nothing to end the solo | `lock`, `solo` |
| `keyframe` | `pointer`, `time`, optional `value` (the current value when omitted), `ease`, `timeline`, `remove` | `keyframe DIR POINTER TIME [VALUE]` |
| `import` | `file`, optional `id`; copies the file into `assets/` and declares the asset | `import DIR FILE` |
| `translate` | `pointer` to a text of the manifest, `language`, `text` (an empty or missing one removes that language), `add` (adds a language the text lacks, starting from the text in the default language, and leaves one it has alone) | `translate DIR POINTER LANGUAGE TEXT`, `--remove` or `--add` |
| `thumbnail` | `file`, `animated`; copies a picture into `assets/` as `previews.thumbnail` or `previews.animated` (`preview` in a first-version manifest) | `thumbnail DIR FILE [--animated]` |
| `upgrade` | optional `rating`, `reasons`, `photosensitive`, `derived_from`; moves a first-version manifest to `wallpaper-rs.wpak/2` | `upgrade DIR [--rating R] [--reason R] [--photosensitive B] [--derived-from A]` |
| `undo`, `redo` | none | `undo`, `redo` |

A new layer gets an id made from its kind (`text`, `text_2`…) and sensible defaults; a particles
layer without a `source` also gets a new system. `keyframe` turns a plain value into
`{"keys": [[time, value]]}`, adds or replaces the key at that time (to the millisecond), keeps the
keys sorted and turns the value back into a plain one when its last key goes; it refuses a value
that follows another source. Keyframe times are seconds on the named timeline, or on the scene
clock without one.

`import` copies pictures, videos, sounds, fonts and `.cube` tables. It refuses a file over the
limits of the format (256 assets, 1 GiB per file, 512 MiB of pictures once decoded, sounds of 8
MiB and 60 s at most and 180 s for all of them, tables of 16 MiB) or an asset id in use before it
copies anything. The file is copied in blocks to a hidden file in `assets/` that is renamed when
complete. A file with the same name and other content is stored as `name-2.ext`; one with the same
size and SHA-256 is reused. The app imports, builds and applies on a worker thread, one task at a
time, so the window keeps answering. `prune` deletes the files of `assets/` that the scene, its
previews and its credits no longer name.

## History

Each action that changes something is one step `{label, undo, redo}`. Only gestures merge: the
app marks the changes it sends while a slider or a canvas handle is dragged, and such a change
merges into the previous gesture with the same label, recorded less than 500 ms before, when its
operations only replace values that step set. Dragging is one step, while typed values, moves,
insertions, removals and every `wprs-ctl scene` command are steps of their own. A new step drops
the steps that were undone; an action that changes nothing once the computed fields follow it,
such as unticking an input the scene uses, records no step. The history keeps at most 500 steps
and 64 MiB of values, dropping the oldest first.

The history is stored with the unsaved document in
`$XDG_STATE_HOME/wallpaper-rs/editor/<hash of the folder>.json` (mode `0600`, folder `0700`),
together with a hash of the files it started from. The app writes it every 30 seconds while
something changed and when the editor closes; `wprs-ctl scene` writes it after each command, so
undo and redo work across commands and between the app and the command line. When a folder opens
with unsaved changes based on the same files, the app offers to recover or discard them, and
`wprs-ctl scene recover [--discard]` does the same. Saving refuses to overwrite files that changed
since the folder was opened. The file goes away when the editor closes with everything saved or
the history is empty; a new one deletes those untouched for 30 days and keeps the 32 newest.
`wprs-app --check` keeps them in a temporary folder.

## Inspector

The inspector is generated from the JSON Schema of scenes (`wprs-ctl scene schema`) and of user
properties (`--property`), produced by `schemars` from the types of `wprs-scene`, so a field added
to the format appears without changes to the editor. `wprs-ctl scene fields DIR POINTER` prints
the fields it shows for a pointer, as JSON. Annotations in the schema guide the widgets:

| Annotation | Meaning |
|------------|---------|
| `x-unit` | unit shown after the label (`px`, `°`, `s`) |
| `x-range` | `[min, max]` of a slider |
| `x-ref` | the value names an `image`, `video`, `sound`, `font` or `lut` asset, a `layer`, a particle `system`, a `timeline` or a user `property`, offered in a list |
| `x-key` | a keyframe `[time, value, ease]` |
| `x-curve` | a list of `[t, value]` points plotted as a curve |

Numbers become sliders when they have a range, colors a color picker, enumerations and tagged
variants a list with translated names, pairs two fields, `x-curve` lists a curve, color stops
`[t, color]` rows of a time and a color picker, fixed pairs of points (a box, a line) two rows of
coordinates, objects a group of fields and other lists a count with *Add* and *Edit…*. The app
names every field, value, effect and easing in the language of the desktop; a test fails when the
schema gains one without a name. Effect parameters come from the descriptor of the effect. A
field that may follow a source (keyframes, a user property, the sound analysis or the pointer) has
a menu to add or remove a keyframe at the current time, bind it, make it fixed again or edit the
binding itself (timeline, loop, scale, offset, clamp, smoothing). An optional field that is set
can go back to its default.

## Package details

The manifest is edited in the same inspector: `/manifest` shows the fields of the JSON Schema of
manifests (`wprs-ctl scene schema --manifest`), generated by `schemars` from the types of
`wprs-format`, title first, then description, change notes, version, id, authors, license,
content, photosensitive, origin, credits, previews, links, tags, inputs, requires, default
language and presets. `schema`, `kind`, `entry`, `strings`, `preview` and the user properties,
which have their own list under *User properties*, are not shown. A first-version manifest shows
only the fields that version knows (title, description, version, id, authors, license, preview and
tags). The default language has no way back to a default, since texts in several languages need
it. More annotations guide the widgets:

| Annotation | Meaning |
|------------|---------|
| `x-markup` | a text with the markup of `package.md`, edited on several lines |
| `x-required` | required in `wallpaper-rs.wpak/2` (`content`, `photosensitive`, `origin`), so it has no way back to a default; a switch that is not set yet shows *No* and *Yes* |
| `x-computed` | filled by the tools (`requires.features`), shown read-only |
| `x-ref` | `thumbnail` or `animated`: a file chooser that runs `thumbnail`; `language`: the languages of the texts and of the desktop, offered in a list |

Texts (titles, descriptions, change notes, labels, help, options and preset labels) show one row
per language, a plain text under the default language. `translate` takes only a pointer whose
schema is a text and writes where the text lives: a plain text stays plain in the default language
or with an empty `language`, and another language turns it into a map with the plain text under
`default_locale` (set to `en` when missing); a map gets or loses that key, and the last text of a
title cannot go; an `"@key"` text writes `strings.<language>.<key>`. A text that does not exist yet
starts in the default language. An empty text removes that language, or an optional field when it
is plain. *Translate* in the app sends `add`, which never overwrites a text. A first-version
manifest only edits its plain texts and the maps it already has; texts in several languages wait for
`upgrade`. Lists of names (authors, tags, the paths of a credit) are edited one per line, lists of
choices (inputs, the reasons of a mature rating) as check boxes, and previews with a file chooser.

`thumbnail` copies a png, jpeg or webp of at most 1024 pixels a side and 1 MiB as it is, and turns
a larger picture, or a gif, into a jpeg of at most 1024 pixels as `assets/preview.jpg`, lowering
the quality (85, 75, 65) and then the side (768, 512 pixels) until it fits in 1 MiB, or refusing
the picture; an animated preview must already be a gif or webp of at most 640 pixels a side, 15 s
and 8 MiB. Files with the same name and other content are kept as `preview-2.jpg`, and `prune`
deletes the ones no longer named. A preview over its size points at its own field.

Every step of the history ends with the inputs and features the document needs, computed as
`wprs-ctl pack build` does: the inputs the scene uses are added to `inputs` (the ones an author
declares, such as `media` or `network`, stay), an `audio`, `pointer` or `clicks` input that the
scene used before the step and no longer uses goes, and `requires.features` becomes exactly the
computed list, keeping `requires.engine` and a `requires` the author added empty to set it. Undo
and redo restore both with the rest of the step, and setting,
adding, removing or moving anything under `/manifest/requires/features` is refused. A folder
edited by hand may lack them: a missing input is an error and a missing feature a warning, and
`build` and `apply` first record the step `sync` that fills them and save. Problems also cover the
rules of `features::check` and its warnings (a declared `audio`, `pointer` or `clicks` input that
nothing uses) and previews over their size, format or length. Every field that version 2 requires
and is missing is a problem of its own, listed together.

A first-version manifest (`wallpaper-rs.wpak/1`) shows a notice with *Upgrade*. `upgrade` does
what `wprs-ctl pack upgrade` does, with the same code (`wprs_format::upgrade_json`), as one step:
it sets `schema` to `wallpaper-rs.wpak/2`, moves `preview` to `previews.thumbnail`, sets
`default_locale` when the texts are in several languages (`en` when all of them have English, or a
language they all share) and renames the property ids that version 2 no longer accepts (`snow-speed`
becomes `snow_speed`, `snow_speed_2` when taken), together with the bindings and triggers of the
scene, `visible_if`, groups and presets that name them. It never guesses the rating, the
photosensitivity or the origin: `rating`, `reasons`, `photosensitive` and `derived_from` set them
(the origin is original with a rating and no `derived_from`), and what is not given stays as
problems until the author sets it.

## Preview

The app asks the daemon for a virtual output named `preview:<n>` with `RequestStream` and shows its
PipeWire stream on the canvas, as the Plasma wallpaper does. Every change sends the unsaved document
with `PreviewScene` (`dbus.md`), at most once per 50 ms, with `solo` applied; a paused or scrubbed
timeline holds the scene at that second, so a still preview renders only when something changes. A
new document replaces the preview without a crossfade, and a playing preview keeps its time; package
details that are still incomplete, such as a missing rating, do not stop it. *Show on the desktop*
previews on every screen the same way. The app repeats the call every 30 seconds while a preview is
on and ends it when the editor closes; the daemon ends it 60 seconds after the last call. A
minimized window marks the preview output covered. `wprs-ctl scene preview DIR [-o OUTPUT] [--time
T]` shows the saved folder for 60 seconds, and `--stop` ends it.

The canvas draws the outline of each layer with handles to move, turn and scale the selected one,
and guides for 16:9, 21:9, 32:9, 4:3, 16:10 and 9:16 screens. Ctrl and the wheel zoom, the wheel
or a drag with the middle or right button pans.

## Starting points, building and applying

`wprs-ctl scene new DIR --from empty|particles|parallax|clock [--id ID] [--title TITLE] [--locale
LANGUAGE]` and the *Scenes* page of the app create a folder from a starting point, with the id
`org.example.<folder name>` (accents dropped, so `Canción` becomes `cancion`) and the folder name as
title unless given: an empty canvas, falling particles with a density property, layered hills that
follow the pointer, or a clock with a color property. The manifest is a valid
`wallpaper-rs.wpak/2` one: `default_locale` is the language of the title, the first language of the
desktop unless given (`es` for `es_ES.UTF-8`), or `en` when the labels of the properties, in English
and Spanish, lack it; `content` `general`, `photosensitive` `false`, `origin` `original`, license
`CC0-1.0`, and the inputs and features the scene uses.
`wprs-ctl scene open DIR --from FILE.wpak` extracts a scene package into an empty folder, which is
how the app edits a copy of a scene from the library. The copy drops `manifest.json.minisig`,
which signed the manifest of the original author, and gets the id `<id>.copy`, so it never takes
the place of the original in the library. A copy of a version 2 package is derived: its origin
credits the original (its authors, or its id without them, its title, license and home page) after
the works the original came from, and its authors stay for whoever edits it to fill in.

`validate` lists the problems of the document and of the folder with the pointer of the field
concerned, nested layers included, the same messages the app shows; unused files and a signature
left by an earlier build are warnings. `build` refuses unsaved changes and errors, signs
`manifest.json` with `--key` exactly as `wprs-ctl pack sign` does, or removes an earlier signature
without it, and packs the folder with the code of `wprs-ctl pack build` into `-o FILE` or
`<id>-<version>.wpak` next to the folder. `apply` builds the package into
`$XDG_DATA_HOME/wallpaper-rs/scenes/`, imports it into the library and shows it on an output, or
on every output.
