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
| `bone` | `layer` (a mesh layer, see "Mesh layers"), `name` (1 to 64 characters without `/` or `#`, not used by another node of the file), optional `parent` (an empty one hangs it from no other bone), `at` (where its head goes, in fractions of the layer; the middle for a new bone), `to` (a new name; the clips, animation layers, `ik` chains and targets, `physics`, `rules`, `hit` and the `attach` of other layers that name the bone follow), `remove` (its children then hang from its parent, which takes its weights); adds, moves, rehangs, renames or removes the bone, at most 128, keeping the place of its children, and saves the mesh as a new file, so that undo returns to the previous one. In the same step, removing a bone drops what names it: its keys in clips (a clip left without keys and without an animation of the file keeps one at rest on the parent), its place in the bones of animation layers (a layer that only moved that bone goes), the `ik` chains that reach for it, its limits and its place in chains (a chain left with fewer than 2 bones or no longer hanging goes), its `physics`, the `rules` that name it, its place in `hit` and the `attach` of the layers that follow it; rehanging a bone, also by dropping it in the tree, drops the chains it unlinks and the `rules` it would close into a loop | `bone DIR LAYER NAME [--at X,Y] [--parent P \| --root] [--rename NEW] [--remove]`, and `bones DIR LAYER` prints the bones, the other nodes (`points`: attachment points and the nodes of parts), parts, blend shapes and their defaults, expressions, animations and clips, and whether the weights are automatic |
| `mesh_depth` | `layer` (a mesh layer, see "Mesh layers"), `points` (a stroke of 1 to 1 024 points, in fractions of the layer), `radius` (above 0 and up to 1 of the longer side) and `by` (−1 000 to 1 000 pixels towards the viewer at the middle of the brush, fading to nothing at its edge; below zero lowers); raises or lowers the depth of the vertices along the stroke, keeps their bones, weights and blend shapes and saves the mesh as a new file, so that undo returns to the previous one. The inspector of the `mesh` field paints it with Paint depth on the preview, the pixels and the brush size; the depth moves vertices with the perspective of `depth` and decides which vertices cover which | `mesh-depth DIR LAYER X,Y;X,Y;... --by PX [--radius R]` |
| `weights` | `layer`, `bone` (the bone painted), `points` (a stroke of 1 to 1 024 points, in fractions of the layer), `mode` (`add`, `subtract`, `smooth` towards the mean of the neighbouring vertices, or `normalise`, which drops the weights under the strength), `by` (the strength at the middle of the brush, 0 to 1), optional `radius` (0.1 of the longer side), `falloff` (0.5: how much of the brush fades towards its edge, 0 hard to 1 soft) and `lock` (bones whose weights the stroke keeps; the painted bone may not be one); each vertex keeps at most 4 bones whose weights add up to 1, the others making room or taking what the painted bone gives up (its parent when no other bone holds the vertex); the weights of the mesh then stay as painted when bones change, and the mesh is saved as a new file | `weights DIR LAYER BONE X,Y;X,Y;... --mode M --by V [--radius R] [--falloff F] [--lock BONE]...`, and `vertex-weights DIR LAYER` prints each vertex with its place, part and weights |
| `auto_weights` | `layer`; binds every vertex to its nearest bones again (the weights of a vertex fall off with its distance to each bone, from its head to the head of its first child) and makes the weights follow the bones until they are painted | `auto-weights DIR LAYER` |
| `clip` | `layer`, `id`, optional `add` (a new clip, of 2 s unless `length` is given; on a mesh without animations of its own it starts with a key at 0 of the rest of its first bone, or of the order of its first part, so that it is valid), `remove` (also removes the animation layers that play it), `to` (a new id; animation layers follow), `length` (seconds, sets `to`), `loop` | `clip DIR LAYER ID [--add \| --remove] [--rename NEW] [--length S] [--loop L]` |
| `key` | `layer`, `clip`, `time` (seconds from the start of the clip, its `from`, to the millisecond, as the timeline shows them) and one of `bone` with optional `rotation`, `position` (`[x, y]`) and `scale` (`[x, y]`), `shape` with optional `weight`, or `order` (a part) with optional `value`; optional `ease` (`step`, `linear` or `cubic`, written as `step`, nothing or `in_out_cubic`: how the key moves on to the next one), `remove`, `from` (moves the key at that time to `time`, or copies it with `copy`); writes the keys of the clip (`bones.<name>.rotation`, `position` and `scale`, `shapes.<name>`, `order.<part>`); `from` is also in seconds from the start of the clip; a bone keyed without values keys the three of them with the pose the clip gives it at that time, the motion of the file included, and when values are given only the named channels are keyed, removed or moved, so a turn leaves the motion of the file in the others | `key DIR LAYER CLIP T (--bone B [--rotation D] [--position X,Y] [--scale S\|X,Y] \| --shape S [--weight W] \| --order P [--value N]) [--ease E] [--remove] [--from T0 [--copy]]`, and `keys DIR LAYER CLIP` prints its keys row by row, in seconds from the start of the clip, with the length and loop of the clip and the blend shapes and parts of the mesh |
| `expression` | `layer`, `name` (1 to 64 characters), optional `shapes` (`{ "<blend shape>": weight }`, −1 to 1), `to` (a new name; the `shapes` of the layer that set it follow), `remove` (also from `shapes`); sets, renames or removes an expression, at most 64 | `expression DIR NAME [--shape S=W]... [--rename NEW] [--remove]` |
| `ik` | `layer`, and `root` with `tip` to add a chain from a bone down to one that hangs below it (2 to 8 bones, at most 16 chains; it starts reaching for the point where its end rests, in pixels of the box from its top left, so nothing moves), or `index` (0 first) to change a chain, with `remove` to remove it; optional `root` with `tip` (a new chain of bones), `target` (`"cursor"`, `"bone:<node>"`, `"layer:<id>"` or a point `[x, y]`), `limits` (`{ "<bone of the chain>": [min, max] }`, −180 to 180 degrees from its rest), `unlimit` (bones whose limits go), `weight` (0 to 1) and `bend` | `ik DIR LAYER add ROOT TIP`, `ik DIR LAYER set N [--root R --tip T]` and `ik DIR LAYER remove N`, with `[--target cursor\|bone:NAME\|layer:ID\|X,Y] [--limit BONE MIN MAX]... [--unlimit BONE]... [--bend B] [--weight W]` |
| `physics` | `layer`, `bones` (nodes of the file), optional `mode`, `stiffness`, `damping`, `inertia`, `wind` (0 to 1), `gravity` (−10 to 10), `rotation` (`[min, max]` degrees within 180) and `translation` (`[min, max]` pixels), `free` (`rotation` or `translation`: the bounds that go); adds a body for each bone that has none (a `spring`, or a `rope` for a new rope) or changes the given settings of the one it has, at most 64; `remove` removes their bodies; `rope` (2 to 9 points in fractions of the layer) with a single name in `bones` and an optional `parent` adds the bones `<name>_1`, `<name>_2`… along the points, each hanging from the one before and the first from `parent`, saves the mesh as a new file and makes the first one swing as a rope | `physics DIR LAYER add BONE... [--mode M] [--stiffness S] [--damping D] [--inertia I] [--gravity G] [--wind W] [--rotation MIN,MAX] [--translation MIN,MAX] [--free rotation\|translation]`, `physics DIR LAYER rope NAME X,Y;X,Y;... [--parent BONE] [...]` and `physics DIR LAYER remove BONE...` |
| `wind` | optional `angle` (degrees clockwise from the right, where it blows to), `strength` (0 to 10 000 pixels a second), `gusts` (0 to 1) and `off`; sets `canvas.wind` from the angle and strength, keeping the one not given from the wind there is (a bound or keyed wind becomes fixed), and `canvas.gusts`, or removes both | `wind DIR [--angle D] [--strength S] [--gusts G \| --noise G] [--off]` |
| `attach` | `layer`, optional `to` (a `mesh` or `model` layer; the one it follows already when omitted), `bone`, `point` (a node of the file, or `[x, y]` in fractions of the layer for a new point there: a node without a mesh named `<layer>_point`, marked `extras.point`, hanging from the bone that leads the nearest vertex, and the mesh is saved as a new file; a layer that already follows a point of its own on that mesh moves it there instead), `offset` (pixels), `nudge` (`[x, y]` pixels of the box of the mesh layer that the layer moves by, turned into `offset` along its bone or point at the rest pose and what it inherits), `inherit` (`position`, `rotation`, `scale`), `detach`; writes `attach`, keeping the offset and what it inherits while it follows the same layer, and refuses a layer that would follow itself, through other layers too. A point marked `extras.point` that the layer leaves (by `detach` or by following something else) goes with it when no other layer follows it and no chain reaches for it | `attach DIR LAYER [--to MESH_LAYER] [--bone NAME \| --point X,Y\|NAME] [--offset X,Y] [--nudge X,Y] [--inherit position,rotation,scale] [--detach]` |
| `point` | `layer`, `name` (as for bones), optional `at` (fractions of the layer; required for a new point), `parent` (a bone; an empty one hangs it from none), `to` (a new name; the `attach` of the layers that follow it and the chains that reach for it as `bone:<name>` follow), `remove` (also drops those `attach` and chains); adds, moves, rehangs, renames or removes an attachment point, a node without a mesh marked `extras.point` that a new point hangs from the bone that leads the mesh there, and saves the mesh as a new file | `point DIR LAYER NAME [--at X,Y] [--parent BONE \| --root] [--rename NEW] [--remove]` |
| `parts` | `layer`, optional `order` (`{ "<part>": n }`, −1000 to 1000, rounded), `hide` and `show` (parts; shown is the default, so the field goes), `clip` (`{ "<part>": "<part it only shows over>" }`, never itself, also through other parts) and `unclip`; writes `parts` and drops what is left empty | `parts DIR LAYER [--order PART N]... [--hide PART]... [--show PART]... [--clip PART TO]... [--unclip PART]...` |
| `particles` | one of `shape` (`system`, `emitter`, `shape`: the shape of an emitter as the scene writes it; the next index adds an emitter with a `rate` of 10), `point` (`system`, `point` 0 to 7, optional `at` in pixels of the layer, `follow` (`true` follows the pointer, `false` stops), `angle`, `remove` for the last one; earlier points that are missing start at `[0, 0]`), `remap` (`system`, `remap` to add, with `index` to replace one, or `index` and `remove`), `cutout` (`system`, `range` `[start, end, opacity]`, none to remove it), `orient` (`system`, `orient`, `angle` with `fixed`), `perspective` (`system`, `focal`, `depth` and `depth_speed` ranges, or `off` to remove the three), `layer` (`system`, `emitter`, `id` of the layer it emits from, `threshold`, `color`, `inherit`, `jitter`, `refresh`), `inherit` (`system`, `child`, `inherit`, `follow`), `shader` (`system`, `shader`, none to remove it), `collider` (`system`, `collider` to add, with `index` to replace one, or `index` and `remove`), `response` (`system`, `index`, `response`: `bounce`, `slide`, `stop`, `stick` or `kill`, optional `bounce` and `friction`), `force` (`system`, `force` to add, with `index` to replace one, or `index` and `remove`; `keep_distance`, `keep_between` and `remap` operators among them), `boids` (`system`, `boids` with the fields to change, starting from a radius of 40, or `off`), `strand` (`system`, `strand`: `rope` or `trail`, `settings` with the fields to change, or `off`), `texture` (`system`, `texture`, none to remove it), `turbulent` (`system`, `turbulent` with the fields of `init.turbulent` to change, `quiet` to drop its sound band, or `off`), `offset` (`system`, `offset` with the fields of `init.offset_noise` to change, or `off`), `pace` (`system`, optional `emitter`, `max_per_step`, none to remove the limit; every emitter without `emitter`), `lit` (`system`, `lit`, `normal`, a picture, an empty one removes it) and `refract` (`system`, `normal`, required while the system has none, `amount` 0 to 1, 0.3 by default, or `off`); `cutout`, `orient`, `shader`, `strand`, `texture`, `lit` and `refract` take a `layer` to change the render of that particles layer, which starts as a copy of the render of the system. Each edit is refused, changing nothing, when it leaves the scene with a problem it did not have | `particles DIR SYSTEM shape N JSON`, `control-point pN [--at X,Y] [--angle D] [--cursor] [--remove]`, `remap add JSON`, `remap set N JSON`, `remap remove N`, `cutout START,END,OPACITY\|none [--layer L]`, `orient screen\|velocity\|fixed\|none [--angle D] [--layer L]`, `perspective [--focal F] [--depth MIN,MAX] [--depth-speed MIN,MAX] [--off]`, `emit-from-layer N LAYER [--threshold T] [--color] [--inherit] [--jitter J] [--refresh R]`, `inherit N color\|size\|alpha\|velocity... [--follow]`, `shader ID\|none [--layer L]`, `collider add JSON`, `collider set N JSON`, `collider remove N`, `collider response N bounce\|slide\|stop\|stick\|kill [--bounce B] [--friction F]`, `keep distance TARGET [--distance D] [--strength S] [--index N]`, `keep between FROM TO [--strength S] [--index N]`, `keep remove N`, `boids [--radius R] [--separation S] [--alignment A] [--cohesion C] [--max-speed V] [--off]`, `operator remap add JSON`, `operator remap set N JSON`, `operator remap remove N`, `rope [--subdivide N] [--width W] [--uv-scale U] [--uv-scroll V] [--smooth BOOL] [--layer L] [--off]`, `trail [--length N] [--width W] [--segments N] [--subdivide N] [--uv-scale U] [--uv-scroll V] [--textured BOOL] [--layer L] [--off]`, `texture ID\|none [--layer L]`, `turbulent [--forward D] [--scale S] [--noise-speed N] [--offset O] [--speed MIN,MAX] [--phase P] [--audio BAND] [--quiet] [--off]`, `offset [--distance D] [--scale S] [--sign X,Y] [--octaves N] [--noise-scale N] [--noise-speed N] [--off]`, `pace [--emitter N] [--max-per-step N]`, `lit [--normal N] [--off] [--layer L]`, `refract [--normal N] [--amount A] [--off] [--layer L]` (`--show` on either prints the two-sided preview, the displacement and what each quality level keeps); `particles DIR SYSTEM handles [--layer L] [--time T]` prints what the canvas draws for the system and `particles DIR SYSTEM budget [--time T] [--quality Q] [--layer L]` its budget |
| `light` | one of `add` (`light`: `point`, `spot`, `tube` or `directional`, optional `id`, `into` a group and `at`, its position in that group; a new light starts in the middle of the canvas with the defaults of its type: a point light of radius 400 at a height of 120, a spot light of radius 900 at 150 with an intensity of 2, a cone of `[25, 45]` and a tilt of 45°, a tube light of length 400 and radius 240 at 60, a directional light with an intensity of 0.8 and a tilt of 50°; `add_layer` with `kind: light` and the type as `source` does the same), `set` (`layer` and any of `light`, which drops `cone`, `length` and `texture` where the new type has none, `color`, `intensity` 0 to 100, `radius` 1 to 100 000, `z`, `cone` `[inner, outer]` for spot lights, `length` for tube lights, `tilt` −89 to 89, `at`, a point of the canvas the light moves to, and `aim`, a point of the canvas it turns to, both through its parents as the light is placed at the start of the scene), `remove` (`layer`), `solo` (`layer`, or none to end it), `mute` (`layer`, `muted`), `volumetric` (`layer` of a point or spot light, optional `density` 0 to 1 and `samples` 4 to 64, starting from 0.2 and 16, or `off`) and `projection` (`layer` of a spot light and one of `asset`, a picture or video of the scene, `from`, a layer that takes no light and reads none that does, or `off`); solo and mute are kept in `/scene/ext/editor/lights` and only change the preview; `set` also drops `volumetric` from tube and directional lights | `light DIR LAYER add [--type T] [--into G] [--at X,Y]`, `light DIR LAYER set [--type T] [--color C] [--intensity I] [--radius R] [--z Z] [--cone IN,OUT] [--length L] [--tilt D] [--at X,Y] [--aim X,Y]`, `light DIR LAYER remove`, `light DIR LAYER solo [--off]`, `light DIR LAYER mute [--off]`, `light DIR LAYER falloff [--time T]` (the points of the falloff curve), `light DIR LAYER volumetric [--density D] [--samples N] [--off]` or `--show` (the glow against the distance, the size and samples each quality level keeps and on phones), `light DIR LAYER projection [--asset A \| --layer L] [--off]` or `--show` (the pictures, videos and layers it may project and where the picture lands on the canvas), and `lights DIR [--time T]` prints every light with its handles, its dirty box and the count against each device tier, or with `--cost [--quality Q]` what the lighting costs at each quality level |
| `lighting` | `layer` (an image, video or text layer), optional `normal`, `env`, `roughness_map` and `emissive_map` (pictures of the scene; an empty id removes one), `roughness`, `reflection` and `strength` (0 to 1), `emissive` (0 to 16) and `off` (removes `lighting`); the first change writes the block | `lighting DIR LAYER [--normal ID] [--env ID] [--roughness-map ID] [--emissive-map ID] [--roughness R] [--reflection R] [--emissive E] [--strength S] [--off]` |
| `normals` | `asset` (a picture) or `layer` (an image layer, whose picture is used), optional `strength` (0 to 16, 2 by default), `blur` (0 to 32 pixels, 1 by default) and `id`; makes a normal map from the edges of the brightness of the picture (a Sobel filter after a Gaussian blur, the same as `wprs-depth` offers), saves it as `normals/<id>-<n>.png`, declares it as a `linear` asset and, with `layer`, sets it as the `normal` of that layer | `normals DIR ASSET --generate [--strength S] [--blur B] [--layer L] [--id ID]`, or `--preview` to print the relief, the map and a lit sphere without keeping anything |
| `paint_map` | `layer`, `map` (`roughness` or `emissive`), `radius`, optional `hardness` and `value` (the gray, 0 to 1) and `points` (fractions of the layer); paints the stroke into the map the layer has, or into a new one the size of its picture (at most 512 on the longer side), white for roughness and black for glow, saves it as `maps/<id>-<n>.png`, declares it as a `linear` asset and sets `roughness_map` or `emissive_map` | `paint-map DIR LAYER roughness\|emissive --points X,Y;X,Y;... [--radius R] [--hardness H] [--value V]`, or `--heat` to print the map as a heat view |
| `ambient` | optional `ambient` and `sky` (colors), `clear` (removes both); sets `canvas.ambient` and `canvas.sky` | `ambient DIR [--ambient C] [--sky C] [--clear]`, or `--preview` to print the hemisphere they make |
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
previews and its credits no longer name, and the meshes of `meshes/` that neither the scene nor the
history names.

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
| `x-ref` | the value names an `image`, `video`, `sound`, `font` or `lut` asset, a `layer` (for the `layer` shape of a particle emitter only the layers that may feed its system), a particle `system`, a `control` point of the system (`p0` and the `points` it has), a `timeline`, a user `property` that holds a value or, for the `trigger` of a timeline, `start`, `beat` or `prop:<id>` of a button or of such a property, or a built-in `effect`, offered in a list; properties show their labels and effects their translated names |
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

### Mesh layers

The tools for mesh layers edit the file of the layer as a whole, whatever its number of parts, as
long as each mesh of the file is drawn by one node and there are at most 64 primitives. Bones are
the joints of its skins and the nodes without a mesh, except attachment points (`extras.point`);
the parts of a mesh without a skin are bound to the bones as soon as there is one. Each edit writes
one skin for every part, with the vertices
at their rest place in the layer and each bone bound where it rests, keeps the other contents of the
file (pictures, materials, blend shapes, animations, names) and drops what nothing uses any more.
The skin records in `extras.weights` whether its weights are `auto` (they follow the bones: adding,
moving or removing a bone binds every vertex again) or `painted` (they stay; a new bone starts with
no weight and a removed one leaves its weights to its parent). A file whose skin says nothing keeps
its weights, as painted. When the folder is saved, the files of earlier edits that the history
still names wait in the state folder of the editor (next to the saved history, outside the scene
folder) and come back when undo or redo names them again, and those that nothing names any more
are deleted, so the folder and the package keep only the meshes in use; a new file never takes the
name of one that waits.

The inspector of the `mesh` field shows the bones as a tree: a click picks a bone, also on the
preview, a double click or the pencil renames it, and a bone dropped on another hangs from it, or
from none when dropped below the list. *Place bones on the preview* drags their heads. *Paint
weights on the preview* paints the picked bone with the brush, its strength, size and soft edge,
keeps the bones that are ticked as locked, and colours the triangles of the mesh by the weight of
the picked bone (from blue for none to red for all of it, blended between the vertices) or by the bone
that leads each triangle, with its colour, drawn as one picture of at most 384 pixels a side that
is made again only after an edit; *Auto weights* runs `auto_weights`. The inspector also picks the clip the scene plays (`animations`)
and `hit` bones and `depth` with its scale and back-face hiding, as `set` on those fields, which
`wprs-ctl scene set` does as well. The blend shapes have sliders that only change the preview, and
an expression is saved from them, shown to be changed and saved again, renamed or removed.

*Make a chain on the preview* takes a click on the bone a chain starts from and one on the bone it
ends at, and adds it with `ik`. Each chain is listed with what it reaches for (the pointer, with a
note that it needs the pointer permission, a point, a bone or point of the mesh, or another layer),
a button that bends it the other way, its weight and a button that limits or frees each of its
bones. On the preview a dashed line joins the end of each chain to its target, a chain that reaches
for a point has a ring that is dragged to move it, and each limit is an arc around its bone from the
smallest to the largest angle, with a dot at each end that is dragged to change it. *Pick bones
that swing on the preview* adds or removes the body of the bone clicked, and each body is listed
with its mode, sliders for its stiffness, damping, inertia, gravity and wind, and ticks that bound
its turn and how far its head slides, each with a slider from its smallest to its largest value
(`free` when unticked); on the preview each body has a ring at its head, an arrow for its gravity
with a square at its tip that is dragged up or down to set it, and a bounded turn is an arc from
its smallest to its largest angle with a dot at each end that is dragged to change it. *Draw a rope on the preview* turns a drag
into a `physics` rope of up to 9 new bones that hangs from the picked bone, or the nearest one.
*Simulate* plays the preview with the physics, or holds it without (the preview drops the
`physics` of the layer, only there), and *Reset* sends the layer once without its bodies so that
they start again from the pose. The wind has sliders for its angle, strength and gusts and *Show
on the preview* draws it as an arrow from the middle of the canvas whose tip is dragged; all of
them run `wind`. *Attach layers on the preview* lists the other layers as chips: one dragged onto a
bone of the mesh follows it, one dropped elsewhere on the mesh follows a new point there, and a
picked chip does the same with a click; what the layer inherits is ticked below, the layers that
follow the mesh are listed with a button that lets them go, and a dotted line joins each of them to
its bone or point, ending in a ring that is dragged to move the layer along it (`nudge`). The
attachment points of the mesh are listed with their name, which is edited in place, the bone they
hang from and a button that removes them; *Add a point* places one in the middle, and on the
preview each point is a square that is dragged to move it, all with `point`. The parts of a mesh of several parts are listed from back to front: a part is
dragged by its handle to a new place, which writes the `order` of every part with `parts`, ticked
to be shown, and given the part it only shows over; the picked part and the part it is cut to are
outlined on the preview.

In the inspector every field that names a bone, a point, a part or a blend shape offers those of
its mesh layer: a single bone (`physics`, `rules`, the `attach` of a layer, which names the bones
and points of the layer it follows) or part (`clip_to`) is a list to pick from, a list of bones
(`chain`, the `bones` of animation layers, `hit`) a set of ticks in the order of the skeleton, a
map keyed by bone or part (`limits`, the `bones` of clips, `parts`, `shapes`) a list to add a key
from with its fields below, and the `target` of a chain a list of the pointer, the bones and
points, the other layers and a point. The schema marks these fields with `x-ref` set to `bone`,
`point`, `part`, `shape` or `reach`; the names come from the file of the mesh, read again only when
it changes.

*Edit clips on the timeline* swaps the timelines under the preview for the clips of the layer: a
clip is picked, added, renamed or removed, and given its length and loop; the playhead scrubs,
plays and loops the preview, which plays that clip alone with the sliders of the blend shapes. The
rows are the channels of the bones that have keys and of the picked bone, the order of the parts
that have keys and every blend shape with a slider that keys its weight at the playhead; a key is
picked, dragged to move it (with Ctrl to copy it), given its easing or removed. On the preview the
picked bone has handles to move, turn and scale it at the playhead, in the space of its parent as
the clip keys it, and *Key* writes, with `key`, the channels the handles changed, or the whole pose
of the bone at the playhead when none did. The timeline shows the seconds from the start of the
clip, its `from`, and the keys are written where the playhead is. Only the preview sees
the clip and the sliders chosen here; the scene keeps its own `animations` and `shapes`.

### Particle systems

Selecting a particle system, or a particles layer, draws its tools over the preview in pixels of
the canvas, through the transform of the layer: every emitter shape with handles (the point; the
corners of a box and the center and radius of a circle, around the control point they are anchored to;
the ends of a line; for `around`, the center, a handle for the radius and one for the end of the
arc, with a dot at each place a birth takes; for `between`, its two ends, a handle that bows the
line and one that slides the widest point of the taper, with dots sized as the taper sizes them;
for `layer`, the outline of the layer and the pixels above the threshold of an image layer, or
only its outline for other layers), the control points `p0` to `p7` as markers to drag, and a ring
whose handle sets the focal length of the perspective. A turbulent start draws, from the middle of
the first emitter, a fan of the directions the noise may give, the directions of a few particles born
now and an arrow with a handle that turns its direction and one on the edge of the fan that widens
or narrows it; a fractal offset draws arrows from a grid of birth places to where the noise moves
them, computed as the engine does, and a ring with a handle for the distance. An end of `around` or `between` that names a
control point moves that point. The inspector of the system lists its emitters with a picker of
the shape, of the control points or the pointer that `around` and `between` follow and of their
count; a picker of the layers it may emit from (never one that holds the system), with the
threshold, its color, its motion and when it is read again; the control points with a toggle to
follow the pointer and a dial for their angle; the remaps of the starting values, each with its
input and output, two-handle ranges for `in` and `out`, its clamps, operation and wave, and the
curve of the output over the input; a cutout with a two-handle range, its opacity and the alpha
ramp it gives; toggles and sliders for the direction, turn, noise speed, spread, phase and speed of
a turbulent start with a picker of the sound band that moves it, and for the distance, size,
detail, scatter, drift and sides of a fractal offset; the orientation with a dial for its angle; the perspective with its focal length
and two-handle ranges for the depth and its speed; what each child takes from its parent particle;
and the particle shaders of the package with a warning that the engine checks them and may leave
them out. The curve is a component of its own that plots any `{ points, x, y }` the editor computes,
so other tools can draw their curves with it. `wprs-ctl scene particles DIR SYSTEM handles` prints
the same data: the transform of the layer and its inverse, the points, each emitter with its
handles, the curves and the lists the pickers offer.

The preview also draws the colliders of the system, colored by their response (red where particles
die, green where they stick): the border of the canvas, a plane as a line with a handle on its
point and one that turns the side particles stay on, a circle with handles on its center and radius,
a box with handles on two corners and a segment with handles on its ends, dashed when they keep
particles inside, and a layer as its outline with its field of distances over it (computed from the
picture of an image layer; other layers show only their outline). A force that keeps a distance
draws a ring of that distance around its target, with a handle on the target and one on the ring;
one that keeps particles between two points draws the line between them with a handle on each end,
and an end or a target that names a control point moves that point. The inspector lists the
colliders with a picker of the response (bounce, slide, stop, stick or kill) and the bounce and
friction it uses, whether a circle or a box keeps particles inside, and a picker of the layers it may
collide with; the kept distances with pickers of their targets among the control points and the
pointer and sliders for the distance and the strength; a toggle and sliders for the neighbour radius,
the separation, the alignment, the cohesion and the most speed of a flock; the remap operators with
the same editor as the remaps of the starting values and `age` as one more input; and the rope and
the trail of the render with their fields, live on the preview, and a picker of the texture among
the image assets. When a rope is on and an emitter may start more than one particle a step, a note
offers to set `max_per_step` to 1 on every emitter, which keeps the rope even; `wprs-ctl scene
particles DIR SYSTEM rope` prints the same advice.

The particle budget of the inspector simulates the system on the processor up to the time of the
timeline (at most 10 seconds), as the CPU path of the engine does, and tells how many particles live
of its capacity, the slots it keeps on the GPU and on the processor for each device tier (phones and
older computers fall on the two lower ones) with the trail points, neighbours of a flock and rope
segments each keeps, and what each quality level keeps of births, neighbours, trail points and rope
points between particles. When asked, the simulated particles, the rope and a few trails are drawn
over the preview, and clicking a particle draws the neighbour radius of the flock around it and
marks its neighbours. A picker of the quality level, Q0 to Q3, caps the quality of the preview, and
*Restart systems* starts only the particle systems of the preview again, keeping the scene and the
rest of its state; `wprs-ctl scene preview DIR --quality Q --restart` does the same, and `wprs-ctl scene particles DIR SYSTEM budget` prints the
budget as JSON.

### Lights

The Add layer menu offers Light with its four types. Every light shows over the preview as a marker
to drag, and the selected one draws its handles, in pixels of the canvas through its parents: a ring
at its radius with a handle on it, a square above it whose distance sets the height `z` (half a pixel
of the preview for each pixel of height), for spot lights a handle that turns it and the edges of the
outer and inner cone with a handle on each, for tube lights a handle on each end that moves, turns and
stretches it, and for directional lights an arrow with a handle that turns it. A check box draws the
box a moving light redraws. These handles change the fields of the light with `set`, so a light whose
fields have keyframes is keyed at the current time, and lights go on the timeline like other layers.
The inspector of a light picks its type and color, slides its intensity, radius, height, length and
cone, plots its light against the distance with the curve component and toggles solo and mute.

Selecting the canvas lists every light with its type and solo and mute toggles, how many lights the
scene has against the 12 it may have and how many each device tier keeps (8 on T0 and T1), the
pickers of the light around and from above with a sphere lit by them, and the preview quality picker
of the particle budget, which here shows for each level whether the lighting is worked out once (Q0)
or how many pixel lights it shades each frame while a light moves.

The inspector of an image, video or text layer has a Lighting part: a switch that writes or removes
`lighting`, pickers of the normal map, the reflected picture and the roughness and glow maps, sliders
for roughness, reflection, glow and strength, Generate normals (an image layer), a dialog with
sliders for strength and blur that shows the relief of the map lit from the top left, the map itself
and a lit sphere before it keeps the map, and Paint roughness and Paint glow, which paint on the layer
in the preview with a brush size, hardness and gray while the map shows over it as a heat view.
`wprs-ctl scene lights`, `light … falloff`, `normals --preview`, `paint-map --heat` and
`ambient --preview` print the same data.

A point or spot light has a Light the air part: a switch for `volumetric`, sliders for density and
samples, the glow plotted against the distance, and for each quality level whether the air is off
(Q0), worked out at a quarter of the size with half the samples (Q1) or at half the size (Q2 and Q3),
with a note that phones work at a quarter; the preview shows the glow live at the picked level, and
the selected light shades the ring of the lit air. A spot light has a Projects picker that lists the
pictures and videos of the scene and the layers it may project without a loop, with a thumbnail of
the picture; the preview draws the picture inside the cone where it lands on the canvas, with a
dashed outline. The list of lights marks lights that light the air or project, and the cost of each
quality level adds the samples of the air, the lights shaded on particles and the pixels refraction
reads, with the same rows for phones.

The particle tools have a Light part: Take the lights of the scene, a normal map picker and a preview
of a sprite lit from the front and from behind, and Bend what is behind the particles with a normal
map picker, an amount slider, the displacement map, the shift each quality level keeps and a switch
that draws the displacement over each particle of the processor simulation on the preview. The
particle budget lists how many lights light the particles and whether they refract at each level.
`wprs-ctl scene light … volumetric --show`, `projection --show`, `particles … lit --show`,
`refract --show` and `lights --cost` print the same data.

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
T] [--quality Q] [--restart]` shows the saved folder for 60 seconds, at most at that quality level and
started again, and `--stop` ends it. The document of a preview may carry `preview: { quality,
restart }`, which the daemon reads only for previews.

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
