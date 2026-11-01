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
| `add_layer` | `kind`, optional `id`, `name` (the name of the layer in the author's language), `source` (the asset of an image, video or sound layer, the system of a particles layer), `into` (a group), `at` (0 is the bottom) | `add-layer DIR KIND [--name N]` |
| `remove_layer`, `duplicate_layer` | `id`; a copy gets new ids for itself and its children | `remove-layer`, `duplicate-layer` |
| `move_layer` | `id`, `to`, optional `into`; without `into` the layer goes to the top level | `move-layer DIR ID TO` |
| `group`, `ungroup` | `ids` of layers with the same parent, each taken once, and an optional group `id`; the `id` of a group, refused while the group has an opacity, transform, anchor, blend, mask, effects, parallax, visibility or clip of its own that ungrouping would lose | `group`, `ungroup` |
| `hide` | `id`, `hidden`; sets `visible`, or `play` on a sound layer, to `false`, keeping what it held under `hidden`, and puts it back when shown | `hide DIR ID --hidden BOOL` |
| `lock`, `solo` | `id` and `locked`; `id`, or nothing to end the solo | `lock`, `solo` |
| `keyframe` | `pointer`, `time`, optional `value` (the current value when omitted), `ease`, `timeline`, `remove`, `from` (moves the keyframe at that time to `time`, keeping its value and easing unless given, as one step; refused when another keyframe is at `time`) | `keyframe DIR POINTER TIME [VALUE]`, `--remove` or `--from T` |
| `import` | `file`, optional `id`; copies the file into `assets/` and declares the asset | `import DIR FILE` |
| `import_effect` | `descriptor`, optional `wgsl` (the `.wgsl` file beside the descriptor when omitted); checks the effect as a package reader does, copies both files to `effects/<id>.json` and `effects/<id>.wgsl` and declares or replaces `effects.<id>` of the scene | `import-effect DIR DESCRIPTOR [--wgsl FILE]` |
| `paint_mask` | `pointer` (a parameter of an effect), optional `channel` (0 red to 3 alpha; every color when omitted), `radius` (a fraction of the shorter side), optional `hardness` (0.5) and `value` (1; 0 erases), `points` (fractions of the layer); paints the stroke into the picture the parameter names, or into a new one of 512 × 512 filled with the `paint` of the parameter (black without one), saves the result as a new `linear` picture `masks/<id>-<n>.png` and points the asset and the parameter at it, so that undo returns to the previous picture. The inspector shows a Paint button, the channel (for `mask` parameters with 3 or 4 `channels`), Erase and the brush size next to every `mask` parameter, and strokes on the layer in the preview are painted | `paint-mask DIR POINTER X,Y;X,Y;... [--channel r\|g\|b\|a] [--radius R] [--hardness H] [--value V]` |
| `mesh_from_picture` | `image` (a picture asset), optional `outline` (follow the opaque part instead of a full grid), `cells` (2 to 64 along the longer side, 16 by default) and `id`; cuts the picture into triangles (cells with any opaque pixel, or the outline of the largest opaque region with points inside it), saves them as a binary glTF `meshes/<id>-<n>.glb` of one part named `body` and adds a `mesh` layer of the size of the picture that shows it. The Add layer menu offers it as Mesh from a picture, next to Mesh for an imported `.glb` | `mesh-from-picture DIR IMAGE [--outline] [--cells N] [--id ID]` |
| `bone` | `layer` (a mesh layer of one part, such as those cut from a picture), `name` (1 to 64 characters without `/` or `#`), optional `parent`, `at` (where its head goes, in fractions of the layer; the middle for a new bone), `remove` (its children then hang from its parent); adds, moves, rehangs or removes the bone, at most 128, binds every vertex to its nearest bones again and saves the mesh as a new file, so that undo returns to the previous one. The inspector of the `mesh` field lists the bones, adds and removes them, picks the clip the preview plays and shows the bones on the preview, where they are dragged; it also adds and removes `ik` chains (built up from the last bone, 2 to 8 bones, reaching for the pointer, a point or a bone, with a note that the pointer needs its permission), `physics` bodies with their mode, `hit` bones and `depth` with its scale and back-face hiding, all as `set` on those fields, which `wprs-ctl scene set` does as well | `bone DIR LAYER NAME [--at X,Y] [--parent P] [--remove]`, and `bones DIR LAYER` prints the bones, parts, blend shapes, animations and clips |
| `mesh_depth` | `layer` (a mesh layer of one part, such as those cut from a picture), `points` (a stroke of 1 to 1 024 points, in fractions of the layer), `radius` (above 0 and up to 1 of the longer side) and `by` (−1 000 to 1 000 pixels towards the viewer at the middle of the brush, fading to nothing at its edge; below zero lowers); raises or lowers the depth of the vertices along the stroke, keeps their bones, weights and blend shapes and saves the mesh as a new file, so that undo returns to the previous one. The inspector of the `mesh` field paints it with Paint depth on the preview, the pixels and the brush size; the depth moves vertices with the perspective of `depth` and decides which vertices cover which | `mesh-depth DIR LAYER X,Y;X,Y;... --by PX [--radius R]` |
| `import_script` | `file`, a `.js` module; copies it to `scripts/<name>` and declares or replaces its entry in `scripts` with the `events` it exports or handles with `on` and the `uses` it imports or calls (`spawn` and `order`, `effect`, `console`, `shared` also for imports of other modules), never its `params`, which are written by hand, refusing scripts that would take the scene past 1 MiB or 64 modules; the author checks what was proposed | `import-script DIR FILE` |
| `translate` | `pointer` to a text of the manifest, `language`, `text` (an empty or missing one removes that language), `add` (adds a language the text lacks, starting from the text in the default language, and leaves one it has alone) | `translate DIR POINTER LANGUAGE TEXT`, `--remove` or `--add` |
| `thumbnail` | `file`, `animated`; copies a picture into `assets/` as `previews.thumbnail` or `previews.animated` (`preview` in a first-version manifest) | `thumbnail DIR FILE [--animated]` |
| `upgrade` | optional `rating`, `reasons`, `photosensitive`, `derived_from`; moves a first-version manifest to `wallpaper-rs.wpak/2` | `upgrade DIR [--rating R] [--reason R] [--photosensitive B] [--derived-from A]` |
| `add_property` | `kind` (`slider`, `bool`, `color`, `combo`, `text`, `file`, `folder`, `button`, `group`, `label` or `divider`), optional `id` (`<kind>_<n>` when omitted), `group`, `label`; the property starts valid, last of its group, and a combo with the options `option_1` and `option_2` and empty labels, so no text in a language the package may not use is written | `add-property DIR KIND` |
| `move_property` | `id`, `to` (its position among the properties of its group, 0 first), optional `group` (an empty one takes it out of its group); the `order` of that group becomes 1, 2… | `move-property DIR ID TO` |
| `rename_property` | `id`, `to`; the bindings, `prop:` triggers, groups, conditions (maps and expressions) and preset values that name it follow; scripts are not rewritten, and the command warns about each script that names the old id in quotes | `rename-property DIR ID TO` |
| `remove_property` | `id`; removes the property, its values in presets and in conditions that are maps, and the `group` of its members; refused, naming each place, while a binding, a `prop:` trigger or an expression condition names it; the command warns about scripts as `rename_property` does | `remove-property DIR ID` |
| `condition` | `id`, `text`, `enabled`: sets `visible_if`, or `enabled_if`, to the expression or, when `text` is a JSON object, to that map; refuses a condition that does not check, and an empty one removes it | `condition DIR ID TEXT [--enabled]` |
| `add_preset` | optional `id` (`preset_<n>` when omitted), `label`; a preset without values | `add-preset DIR` |
| `undo`, `redo` | none | `undo`, `redo` |

A new layer gets an id made from its `name`, or else from its kind (`text`, `text_2`…; accents
are dropped, other characters become `_`, and a name with no Latin letter or digit falls back to
the kind), and sensible defaults; a text layer starts with its name as its text, and a particles
layer without a `source` also gets a new system. The app sends the name of the kind in the
language of the desktop (`Texto` becomes `texto`), and offers an image, video or sound layer only
once there is an asset of that kind. `keyframe` turns a plain value into
`{"keys": [[time, value]]}`, adds or replaces the key at that time (to the millisecond), keeps the
keys sorted and turns the value back into a plain one when its last key goes; it refuses a value
that follows another source. Keyframe times are seconds on the named timeline, or on the scene
clock without one. In the timeline of the app a keyframe is dragged along its track, to the
hundredth of a second, and dropped with `keyframe` and `from`.

`import` copies pictures, videos, sounds, fonts and `.cube` tables. It refuses a file over the
limits of the format (256 assets, 1 GiB per file, 512 MiB of pictures once decoded, sounds of 32
MiB and 10 minutes at most, the ones up to 60 s 180 s together and the longer ones to be streamed,
tables of 16 MiB); MP3 sounds and `.ttc`, `.otc` and `.woff2` fonts are imported too or an asset id in use before it
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
| `x-ref` | the value names an `image`, `video`, `sound`, `font` or `lut` asset, a `layer`, a particle `system`, a `timeline`, a user `property` that holds a value or, for the `trigger` of a timeline, `start`, `beat` or `prop:<id>` of a button or of such a property, or a built-in `effect`, offered in a list; properties show their labels and effects their translated names |
| `x-key` | a keyframe `[time, value, ease]` |
| `x-curve` | a list of `[t, value]` points edited on a plot: a click adds a point, a drag moves it (one gesture) and a double click removes it; `t` goes from `0` to `1` across and the value up to its `x-range`, or a quarter above the highest point |

Numbers become sliders when they have a range, colors a color picker, enumerations and tagged
variants a list with translated names, pairs two fields, `x-curve` lists a curve, color stops
`[t, color]` rows of a time and a color picker, fixed pairs of points (a box, a line) two rows of
coordinates, objects a group of fields and other lists a count with *Add* and *Edit…*. A number
that may also be a name, such as `clock.max_fps` (`display`), has the widget of the number and the
names in `options`, shown as check boxes. The app names every field, value, effect, easing, layer
kind, asset kind, language and computed feature in the language of the desktop; a test fails when
the schema gains one without a name, and another walks the inspector of every starting point with
every layer kind, variant and list item and fails when an option or a shown value has no Spanish
translation or a raw JSON field holds a plain text. Effect parameters come from the descriptor of
the effect, with its `label` as `title` and its `help` as `help` (a tooltip) in the language of the
desktop; a `texture` or `mask` parameter offers the image assets and every other layer, written as
`{ "layer": "<id>" }`, and every entry of `options` is a field of its own, with its values as
choices and bindable to a property. The outline adds `effect_names`, the `name` of each effect of
the package in the language of the desktop. Problems and refused actions show the message of the validator and of the editor, the
same English text that `wprs-ctl scene validate` and the commands print, under a translated
heading; each problem leads to its field, whose name is translated. A field that may follow a
source (keyframes, a user property, the sound analysis or the pointer) has
a menu to add or remove a keyframe at the current time, bind it, make it fixed again or edit the
binding itself (timeline, loop, scale, offset, clamp, smoothing). An optional field that is set
can go back to its default.

## Package details

The manifest is edited in the same inspector: `/manifest` shows the fields of the JSON Schema of
manifests (`wprs-ctl scene schema --manifest`), generated by `schemars` from the types of
`wprs-format`, title first, then description, change notes, version, id, authors, license, content,
photosensitive, origin, credits, previews, links, tags, inputs, platforms, display, requires,
needed packages and default language, and after them the fields the order does not name (such as
`gpu`). `schema`, `kind`, `entry`, `strings`, `preview`, `playback`, `web` and `permissions`, which
scenes do not use, and the user properties and presets, which have their own lists, are not shown.
`display` offers what applies to the kind of the package: `span` and `accent` for scenes, and
`fit`, `background`, `flip`, `zoom` or `position` only for the pictures and videos that use them;
a value already written there by hand stays visible so that it can be removed. A first-version
manifest shows only the fields that version knows (title, description, version, id, authors,
license, preview and tags). The default language has no way back to a default, since texts in
several languages need it. More annotations guide the widgets:

| Annotation | Meaning |
|------------|---------|
| `x-markup` | a text with the markup of `package.md`, edited on several lines |
| `x-required` | required in `wallpaper-rs.wpak/2` (`content`, `photosensitive`, `origin`), so it has no way back to a default; a switch that is not set yet shows *No* and *Yes* |
| `x-computed` | filled by the tools (`requires.features`), shown read-only with translated names but for the features the author chooses |
| `x-ref` | `thumbnail` or `animated`: a file chooser that runs `thumbnail`; `language`: the languages of the texts and of the desktop, offered in a list by their names in the language of the desktop |

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
adding, removing or moving anything under `/manifest/requires/features` is refused, except adding
or removing `locale_files`, which the author chooses (a check box under the computed list). A folder
edited by hand may lack them: a missing input is an error and a missing feature a warning, and
`build` and `apply` first record the step `sync` that fills them and save. Problems also cover the
rules of `features::check` and its warnings (a declared `audio`, `pointer`, `clicks`, `media` or
`network` input that nothing uses), the verdict of `wprs-ctl pack check` for each platform that
refuses the package or plays it without a feature (a warning at `/manifest/requires/features`), a
script that declares nothing but uses more than the first interface (an error at its entry of
`/scene/scripts`), and previews over their size, format or length. Every field that version 2 requires and is missing is a
problem of its own, listed together. Locale files, effect shaders and `.glb` models are read again
only when their size or modification time changes, so editing a value never reads them.

A first-version manifest (`wallpaper-rs.wpak/1`) shows a notice with *Upgrade*. `upgrade` does what
`wprs-ctl pack upgrade` does, with the same code (`wprs_format::upgrade_json`), as one step: it sets
`schema` to `wallpaper-rs.wpak/2`, moves `preview` to `previews.thumbnail`, sets `default_locale`
when the texts are in several languages (`en` when all of them have English, or a language they all
share) and renames the property ids that version 2 no longer accepts (`snow-speed` becomes
`snow_speed`, `snow_speed_2` when taken), together with the bindings and triggers of the scene, the
groups, the maps and expressions of `visible_if` and `enabled_if` and the presets that name them. It
never guesses the rating, the photosensitivity or the origin: `rating`, `reasons`, `photosensitive`
and `derived_from` set them (the origin is original with a rating and no `derived_from`), and what
is not given stays as problems until the author sets it.

## User properties and presets

*User properties* in the *Package* tab lists the properties in the order of the settings form
(`wprs_scene::order`, by `order` and then id, each group followed by its members) and then the
presets, in one list that scrolls. *Add property* offers every kind (`add_property`) and sends no
label, so a new property shows its id until the author gives it one; the arrows move a property
among the ones of its group (`move_property`) and its menu renames it (`rename_property`), moves it
into a group, last, or out of it, just after its group, and removes it (`remove_property`). Renaming
or removing the property that the inspector shows moves the inspector to the new id, or to the
package details. The rename dialog says that scripts are not rewritten and names the scripts that
name the property in quotes; removing such a property asks first. A property shows the fields of the
JSON Schema of properties (`wprs-ctl scene schema --property`) for its kind, label, help, group,
order, icon and conditions first, guided by more annotations:

| Annotation | Meaning |
|------------|---------|
| `x-ref` | `group`: the groups of the manifest, named by their labels, and none for a group, since groups do not nest; `icon`: the icons of `scene.md`; `option`: the values of the options of the combo, named by their labels; `media`: the image and video assets |
| `x-condition` | `visible_if` and `enabled_if`, edited on one line |
| `x-color` | a color picker, with transparency only when `alpha` is set |
| `x-scalar` | a value typed as a number, `true`, `false` or a text |

Labels written as `"@key"` show the text of `strings` in the language of the desktop, else in the
default language, as the settings form does. The default of a slider is a slider from `min` to `max`
with its unit, in whole numbers when `integer` is set; `decimals` go from 0 to 6, `max_len` from 1
to 1024, `every` from 10 to 86400 seconds and `accept` offers `image` and `video` as check boxes.
*Add* under the options of a combo appends an option with an empty label and a value no other option
has. A condition is a JSON object, a map of values, or an expression of the language of `scene.md`;
every keystroke checks it as the validator does, so unknown ids, properties without a value,
comparisons of values of other kinds, conditions that are not true or false and conditions that
depend on themselves through other properties show under the field, and only a condition that
checks is recorded, with `condition`. A problem of a group or a condition points at that field of
the property, and a problem of a preset at its id or at the value it sets.

*Add preset* adds a preset without a label (`add_preset`) and shows it. A preset shows its id and
label and one row per property it may set (not files, folders, secret texts, buttons, groups, labels
or dividers), with the widget of the kind of the property: a row the preset sets shows its value,
the others the default of the property, and *Default* takes a value out of the preset.

A value that follows a combo or a switch with `{"prop"}`, such as the image of a layer, the texture
of a mask, of an effect or of particles, a video or a color, shows the property it follows, among
the properties that hold a value, and under *Map* one row per option (*Off* and *On* for a switch)
with the widget of the value itself, such as the list of images: the variants of `scene.md`. A row
the map lacks is empty, and the engine then reads the value of the property itself.

Adding, changing or removing properties recomputes `requires.features` like any other step:
`properties2` follows the newer kinds and fields of `scene.md` and a spelling the first version
cannot read, `folder` a folder and `user_videos` a file or folder that accepts videos.

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

The canvas draws the outline of each layer with handles to turn and scale the selected one, and
guides for 16:9, 21:9, 32:9, 4:3, 16:10 and 9:16 screens. Dragging a layer that is not locked
selects it and moves it, as one gesture. A drag that starts outside the layers, or any drag with
Shift, draws a rectangle that selects the layers it touches and picks them as Ctrl does in the
layer list, ready for *Group*. Ctrl and the wheel zoom, the wheel or a drag with the middle or
right button pans.

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
left by an earlier build are warnings. With `--json` each problem also carries `said`, a message id
such as `scene.color` or `format.invalid` with its arguments; the app and `wprs-ctl` tell the
messages and refusals in the language of the desktop from `po/<language>/wprs-messages.po`, and the
outline names the steps of `history.undo` and `history.redo` and the layers of the starting points
(`label`) in that language while the ids stay as stored. A starting point made in a language its
labels lack keeps that language, the one of the title, as `default_locale` and copies the English
labels into it. `build` refuses unsaved changes and errors, signs
`manifest.json` with `--key` exactly as `wprs-ctl pack sign` does, or removes an earlier signature
without it, and packs the folder with the code of `wprs-ctl pack build` into `-o FILE` or
`<id>-<version>.wpak` next to the folder. `apply` builds the package into
`$XDG_DATA_HOME/wallpaper-rs/scenes/`, imports it into the library and shows it on an output, or
on every output.
