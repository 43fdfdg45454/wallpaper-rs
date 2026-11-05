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

An `add` at index `-` or `0` of a list that does not exist yet creates the list, as long as the
object that holds it exists; undo removes the list again.

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
| `add_layer` | `kind`, optional `id`, `name` (the name of the layer in the author's language), `source` (the asset of an image, video, sound, mesh or model layer, the system of a particles layer), `file` (a picture, video, sound or `.glb` file of the kind of the layer, imported as its asset in the same step; a `.glb` must read as a mesh for a `mesh` layer and as a model for a `model` layer, and the asset takes the name of the file, or `<id>_file` when that is the id of the layer), `into` (a group), `at` (0 is the bottom); the Add layer menu offers Mesh from a .glb file… | `add-layer DIR KIND [--name N] [--source ASSET \| --file FILE]` |
| `remove_layer`, `duplicate_layer` | `id`; a copy gets new ids for itself and its children | `remove-layer`, `duplicate-layer` |
| `move_layer` | `id`, `to`, optional `into`; without `into` the layer goes to the top level | `move-layer DIR ID TO` |
| `group`, `ungroup` | `ids` of layers with the same parent, each taken once, and an optional group `id`; the `id` of a group, refused while the group has an opacity, transform, anchor, blend, mask, effects, parallax, visibility or clip of its own that ungrouping would lose | `group`, `ungroup` |
| `hide` | `id`, `hidden`; sets `visible`, or `play` on a sound layer, to `false`, keeping what it held under `hidden`, and puts it back when shown | `hide DIR ID --hidden BOOL` |
| `lock`, `solo` | `id` and `locked`; `id`, or nothing to end the solo | `lock`, `solo` |
| `keyframe` | `pointer`, `time`, optional `value` (the value of the keyframe at that time, or the current value, when omitted), `ease`, `timeline` (an existing timeline; refused until the `timeline` action adds it), `remove`, `from` (moves the keyframe at that time to `time`, keeping its value, easing and handles unless given, as one step; refused when another keyframe is at `time`), `base` (a number, list of numbers or color like the keyframes, which are then added to it: relative values; each number from -1000000 to 1000000; removing the last keyframe leaves the base plus its value, added as the scene adds them: a number to each part of a list, and a hex color to a color list of 3 or 4 parts, with the alpha kept up to 1; a base and value that cannot be added refuse the removal, and `absolute` leaves the value alone), `absolute` (drops `base`), `in_handle` and `out_handle` (`[seconds, value]` of the Bézier handle before or after the keyframe of a single number, from 0 s up to the previous or next keyframe and from -1000000 to 1000000 in value, so refused on the first key before it and the last key after it, and a move, insert or removal that would leave a handle past its neighbour is refused too, as is any edit that would leave the scene invalid; they replace its easing, an `ease` replaces them, and a handle that is left out keeps its value), `straighten` (drops the out handle or the easing of the keyframe at `time` and the in handle of the next keyframe, keeps their other handles and is one step; refused together with any other field but `pointer` and `time`); the timeline samples the handles as the scene does, and its Curve and base of the keyframe dialog drags both handles of the curve to the next keyframe, straightens it and sets or drops the base | `keyframe DIR POINTER TIME [VALUE] [--ease E] [--timeline T] [--base JSON \| --absolute] [--in-handle DT,DV] [--out-handle DT,DV]`, `--remove`, `--from T` or `--straighten` |
| `set` on a `trigger` or `play_on` | the trigger of a timeline or of a sound layer; the picker of the inspector offers `start`, `beat`, the buttons and settings of the package (`prop:`) and, in scenes, `media:play`, `media:pause`, `media:track` and `media:cover` with their names | `trigger DIR TARGET [TRIGGER]` (a timeline or sound layer id; prints the trigger without one), `--none` to take it away, `--list` for the offered ones; an unknown `media:` name is refused |
| `import` | `file`, optional `id`, `linear` (the picture holds data such as a normal map or a mask) and `atlas` (`[rows, columns]`, 1 to 256 each, of a sprite sheet), both refused for a file that is not a picture; copies the file into `assets/` and declares the asset; a refused import, or a refused `add_layer` with a `file`, takes back the copy it made | `import DIR FILE [--id ID] [--linear] [--atlas ROWS,COLUMNS]` |
| `import_effect` | `descriptor`, optional `wgsl` (the `.wgsl` file beside the descriptor when omitted); checks the effect as a package reader does, copies both files to `effects/<id>.json` and `effects/<id>.wgsl` and declares or replaces `effects.<id>` of the scene | `import-effect DIR DESCRIPTOR [--wgsl FILE]` |
| `paint_mask` | `pointer` (a parameter of an effect), optional `channel` (0 red to 3 alpha; every color when omitted), `radius` (a fraction of the shorter side), optional `hardness` (0.5) and `value` (1; 0 erases), `points` (fractions of the layer); paints the stroke into the picture the parameter names, or into a new one of 512 × 512 filled with the `paint` of the parameter (black without one), saves the result as a new `linear` picture `masks/<id>-<n>.png` (`.ktx2` with the codec and quality of the mask it replaces when that one is KTX2) and points the asset and the parameter at it, so that undo returns to the previous picture. The inspector shows a Paint button, the channel (for `mask` parameters with 3 or 4 `channels`), Erase and the brush size next to every `mask` parameter, and strokes on the layer in the preview are painted | `paint-mask DIR POINTER X,Y;X,Y;... [--channel r\|g\|b\|a] [--radius R] [--hardness H] [--value V]` |
| `mesh_from_picture` | `image` (a picture asset), optional `outline` (follow the opaque part instead of a full grid), `cells` (2 to 64 along the longer side, 16 by default) and `id`; cuts the picture into triangles (cells with any opaque pixel, or the outline of the largest opaque region with points inside it), saves them as a binary glTF `meshes/<id>-<n>.glb` of one part named `body` and adds a `mesh` layer of the size of the picture that shows it. The Add layer menu offers it as Mesh from a picture, next to Mesh for an imported `.glb` | `mesh-from-picture DIR IMAGE [--outline] [--cells N] [--id ID]` |
| `bone` | `layer` (a mesh layer, see "Mesh layers"), `name` (1 to 64 characters without `/` or `#`, not used by another node of the file), optional `parent` (an empty one hangs it from no other bone), `at` (where its head goes, in fractions of the layer; the middle for a new bone), `to` (a new name; the clips, animation layers, `ik` chains and targets, `physics`, `rules`, `hit` and the `attach` of other layers that name the bone follow), `remove` (its children then hang from its parent, which takes its weights); adds, moves, rehangs, renames or removes the bone, at most 128, keeping the place of its children, and saves the mesh as a new file, so that undo returns to the previous one. In the same step, removing a bone drops what names it: its keys in clips (a clip left without keys and without an animation of the file keeps one at rest on the parent), its place in the bones of animation layers (a layer that only moved that bone goes), the `ik` chains that reach for it, its limits and its place in chains (a chain left with fewer than 2 bones or no longer hanging goes), its `physics`, the `rules` that name it, its place in `hit` and the `attach` of the layers that follow it; rehanging a bone, also by dropping it in the tree, drops the chains it unlinks and the `rules` it would close into a loop | `bone DIR LAYER NAME [--at X,Y] [--parent P \| --root] [--rename NEW] [--remove]`, and `bones DIR LAYER` prints the bones, the other nodes (`points`: attachment points and the nodes of parts), parts, blend shapes and their defaults, expressions, animations and clips, and whether the weights are automatic |
| `mesh_depth` | `layer` (a mesh layer, see "Mesh layers"), `points` (a stroke of 1 to 1 024 points, in fractions of the layer), `radius` (above 0 and up to 1 of the longer side) and `by` (−1 000 to 1 000 pixels towards the viewer at the middle of the brush, fading to nothing at its edge; below zero lowers); raises or lowers the depth of the vertices along the stroke, keeps their bones, weights and blend shapes and saves the mesh as a new file, so that undo returns to the previous one. The inspector of the `mesh` field paints it with Paint depth on the preview, the pixels and the brush size; the depth moves vertices with the perspective of `depth` and decides which vertices cover which | `mesh-depth DIR LAYER X,Y;X,Y;... --by PX [--radius R]` |
| `weights` | `layer`, `bone` (the bone painted), `points` (a stroke of 1 to 1 024 points, in fractions of the layer), `mode` (`add`, `subtract`, `smooth` towards the mean of the neighbouring vertices, or `normalise`, which drops the weights under the strength), `by` (the strength at the middle of the brush, 0 to 1), optional `radius` (0.1 of the longer side), `falloff` (0.5: how much of the brush fades towards its edge, 0 hard to 1 soft) and `lock` (bones whose weights the stroke keeps; the painted bone may not be one); each vertex keeps at most 4 bones whose weights add up to 1, the others making room or taking what the painted bone gives up (its parent when no other bone holds the vertex); the weights of the mesh then stay as painted when bones change, and the mesh is saved as a new file | `weights DIR LAYER BONE X,Y;X,Y;... --mode M --by V [--radius R] [--falloff F] [--lock BONE]...`, and `vertex-weights DIR LAYER` prints each vertex with its place, part and weights |
| `auto_weights` | `layer`; binds every vertex to its nearest bones again (the weights of a vertex fall off with its distance to each bone, from its head to the head of its first child) and makes the weights follow the bones until they are painted | `auto-weights DIR LAYER` |
| `clip` | `layer` (a mesh or a model), `id`, optional `add` (a new clip, of 2 s unless `length` is given; on a mesh without animations of its own it starts with a key at 0 of the rest of its first bone, or of the order of its first part, so that it is valid), `remove` (also removes the animation layers that play it), `to` (a new id; animation layers follow), `length` (seconds, sets `to`), `loop`, `animation` (an animation of the file the clip plays whole), `from` (seconds into the animation where the clip starts, keeping its length within the animation), `play` (with `add`: also adds an animation layer that plays the new clip, at most 16; the clip bar offers it as Add a clip and play it in a new animation layer) | `clip DIR LAYER ID [--add [--play] \| --remove] [--rename NEW] [--length S] [--loop L] [--animation NAME] [--from S]` |
| `key` | `layer`, `clip`, `time` (seconds from the start of the clip, its `from`, to the millisecond, as the timeline shows them) and one of `bone` with optional `rotation`, `position` (`[x, y]`), `scale` (`[x, y]`), `angles` (`[x, y, z]` degrees) and `shift` (`[x, y, z]` units), `shape` with optional `weight`, or `order` (a part) with optional `value`; optional `ease` (`step`, `linear` or `cubic`, written as `step`, nothing or `in_out_cubic`: how the key moves on to the next one), `remove`, `from` (moves the key at that time to `time`, or copies it with `copy`); writes the keys of the clip (`bones.<name>.rotation`, `position`, `scale`, `angles` and `shift`, `shapes.<name>`, `order.<part>`); on a model a bone keyed without values keys `angles` and `shift`; `from` is also in seconds from the start of the clip; a bone keyed without values keys the three of them with the pose the clip gives it at that time, the motion of the file included, and when values are given only the named channels are keyed, removed or moved, so a turn leaves the motion of the file in the others | `key DIR LAYER CLIP T (--bone B [--rotation D] [--position X,Y] [--scale S\|X,Y] [--angles X,Y,Z] [--shift X,Y,Z] \| --shape S [--weight W] \| --order P [--value N]) [--ease E] [--remove] [--from T0 [--copy]]`, and `keys DIR LAYER CLIP` prints its keys row by row, in seconds from the start of the clip, with the length and loop of the clip and the blend shapes and parts of the mesh |
| `expression` | `layer`, `name` (1 to 64 characters), optional `shapes` (`{ "<blend shape>": weight }`, −1 to 1), `to` (a new name; the `shapes` of the layer that set it follow), `remove` (also from `shapes`); sets, renames or removes an expression, at most 64 | `expression DIR NAME [--shape S=W]... [--rename NEW] [--remove]` |
| `ik` | `layer`, and `root` with `tip` to add a chain from a bone down to one that hangs below it (2 to 8 bones, at most 16 chains; it starts reaching for the point where its end rests, in pixels of the box from its top left, so nothing moves), or `index` (0 first) to change a chain, with `remove` to remove it; optional `root` with `tip` (a new chain of bones), `target` (`"cursor"`, `"bone:<node>"`, `"layer:<id>"` or a point `[x, y]`), `limits` (`{ "<bone of the chain>": [min, max] }`, −180 to 180 degrees from its rest), `unlimit` (bones whose limits go), `weight` (0 to 1) and `bend` | `ik DIR LAYER add ROOT TIP`, `ik DIR LAYER set N [--root R --tip T]` and `ik DIR LAYER remove N`, with `[--target cursor\|bone:NAME\|layer:ID\|X,Y] [--limit BONE MIN MAX]... [--unlimit BONE]... [--bend B] [--weight W]` |
| `physics` | `layer`, `bones` (nodes of the file), optional `mode`, `stiffness`, `damping`, `inertia`, `wind` (0 to 1), `gravity` (−10 to 10), `rotation` (`[min, max]` degrees within 180) and `translation` (`[min, max]` pixels), `free` (`rotation` or `translation`: the bounds that go); adds a body for each bone that has none (a `spring`, or a `rope` for a new rope) or changes the given settings of the one it has, at most 64; `remove` removes their bodies; `rope` (2 to 9 points in fractions of the layer) with a single name in `bones` and an optional `parent` adds the bones `<name>_1`, `<name>_2`… along the points, each hanging from the one before and the first from `parent`, saves the mesh as a new file and makes the first one swing as a rope | `physics DIR LAYER add BONE... [--mode M] [--stiffness S] [--damping D] [--inertia I] [--gravity G] [--wind W] [--rotation MIN,MAX] [--translation MIN,MAX] [--free rotation\|translation]`, `physics DIR LAYER rope NAME X,Y;X,Y;... [--parent BONE] [...]` and `physics DIR LAYER remove BONE...` |
| `wind` | optional `angle` (degrees clockwise from the right, where it blows to), `strength` (0 to 10 000 pixels a second), `gusts` (0 to 1) and `off`; sets `canvas.wind` from the angle and strength, keeping the one not given from the wind there is (a bound or keyed wind becomes fixed), and `canvas.gusts`, or removes both | `wind DIR [--angle D] [--strength S] [--gusts G \| --noise G] [--off]` |
| `attach` | `layer`, optional `to` (a `mesh` or `model` layer; the one it follows already when omitted), `bone`, `point` (a node of the file, or `[x, y]` in fractions of the layer for a new point there: a node without a mesh named `<layer>_point`, marked `extras.point`, hanging from the bone that leads the nearest vertex, and the mesh is saved as a new file; a layer that already follows a point of its own on that mesh moves it there instead), `offset` (pixels), `nudge` (`[x, y]` pixels of the box of the mesh layer that the layer moves by, turned into `offset` along its bone or point at the rest pose and what it inherits), `inherit` (`position`, `rotation`, `scale`), `detach`; writes `attach`, keeping the offset and what it inherits while it follows the same layer, and refuses a layer that would follow itself, through other layers too. A point marked `extras.point` that the layer leaves (by `detach` or by following something else) goes with it when no other layer follows it and no chain reaches for it | `attach DIR LAYER [--to MESH_LAYER] [--bone NAME \| --point X,Y\|NAME] [--offset X,Y] [--nudge X,Y] [--inherit position,rotation,scale] [--detach]` |
| `point` | `layer`, `name` (as for bones), optional `at` (fractions of the layer; required for a new point), `parent` (a bone; an empty one hangs it from none), `to` (a new name; the `attach` of the layers that follow it and the chains that reach for it as `bone:<name>` follow), `remove` (also drops those `attach` and chains); adds, moves, rehangs, renames or removes an attachment point, a node without a mesh marked `extras.point` that a new point hangs from the bone that leads the mesh there, and saves the mesh as a new file | `point DIR LAYER NAME [--at X,Y] [--parent BONE \| --root] [--rename NEW] [--remove]` |
| `parts` | `layer`, optional `order` (`{ "<part>": n }`, −1000 to 1000, rounded), `hide` and `show` (parts; shown is the default, so the field goes), `clip` (`{ "<part>": "<part it only shows over>" }`, never itself, also through other parts) and `unclip`; writes `parts` and drops what is left empty | `parts DIR LAYER [--order PART N]... [--hide PART]... [--show PART]... [--clip PART TO]... [--unclip PART]...` |
| `particles` | one of `create` (`system`, optional `from`, a system to copy, `parent`, a system that starts it, and `on`, `static` when omitted: a system without a layer), `delete` (`system`; refused while a layer or another system names it), `settings` (`system`, optional `capacity`, `seed`, `prewarm`, `space`, `rate` and `emitter`, every emitter when omitted; a rate that follows keyframes or another source is refused unless `replace`; `lock`, the values a particles layer cannot change, `none` removing them), `emit` (`system`, `emitter`, `values` of `burst`, `follow`, `inherit`, `duration`, `delay`, `periodic` and `audio_gate` as the scene writes them, `null` or `false` removing one, and `clear`), `window` (`system`, `index` of a force, `window`: four shares of the life in order where it fades in, is full, fades out and is gone, none to remove it), `rename` (`system`, `to`; the layers and systems that use it follow), `start` (`system`, `ranges`: `life`, `speed`, `angle`, `size`, `rotation`, `spin`, `frame`, `color`, `alpha`, `velocity`, `radial`, `hsv`, `palette` and `gradient` as the scene writes them, where one of `hsv`, `palette` and `gradient` replaces the other two, a `null` end or curve of a range keeps the current one and a curve of 1 is dropped, and `clear`: `alpha`, `velocity`, `radial`, `hsv`, `palette` or `gradient`), `child` (`system`, optional `index`, `child` as the scene writes it, `remove`), `over` (`system`, optional `layer`, `size`: `[time, scale]` keys and `color`: `[time, color]` stops, an empty list removes them), `render` (`system`, optional `layer`, `stretch`, `frames` (`none` removes it), `fps`, `mix`, `brightness`, `cycles` (0 removes it) and `order` (`none` removes it)), `shape` (`system`, `emitter`, `shape`: the shape of an emitter as the scene writes it; the next index adds an emitter with a `rate` of 10), `point` (`system`, `point` 0 to 7, optional `at` in pixels of the layer, `follow` (`true` follows the pointer, `false` stops), `angle`, `remove` for the last one; earlier points that are missing start at `[0, 0]`), `remap` (`system`, `remap` to add, with `index` to replace one, or `index` and `remove`), `cutout` (`system`, `range` `[start, end, opacity]`, none to remove it), `orient` (`system`, `orient`, `angle` with `fixed`), `perspective` (`system`, `focal`, `depth` and `depth_speed` ranges, or `off` to remove the three), `layer` (`system`, `emitter`, `id` of the layer it emits from, `threshold`, `color`, `inherit`, `jitter`, `refresh`), `inherit` (`system`, `child`, `inherit`, `follow`), `shader` (`system`, `shader`, none to remove it), `collider` (`system`, `collider` to add, with `index` to replace one, or `index` and `remove`), `response` (`system`, `index`, `response`: `bounce`, `slide`, `stop`, `stick` or `kill`, optional `bounce` and `friction`), `force` (`system`, `force` to add, with `index` to replace one, keeping its `window` unless the new force gives one, or `index` and `remove`; `keep_distance`, `keep_between` and `remap` operators among them), `boids` (`system`, `boids` with the fields to change, starting from a radius of 40, or `off`), `strand` (`system`, `strand`: `rope` or `trail`, `settings` with the fields to change, or `off`), `texture` (`system`, `texture`, none to remove it), `turbulent` (`system`, `turbulent` with the fields of `init.turbulent` to change, `quiet` to drop its sound band, or `off`), `offset` (`system`, `offset` with the fields of `init.offset_noise` to change, or `off`), `pace` (`system`, optional `emitter`, `max_per_step`, none to remove the limit; every emitter without `emitter`), `lit` (`system`, `lit`, `normal`, a picture, an empty one removes it) and `refract` (`system`, `normal`, required while the system has none, `amount` 0 to 1, 0.3 by default, or `off`); `cutout`, `orient`, `shader`, `strand`, `texture`, `lit` and `refract` take a `layer` to change the render of that particles layer, which starts as a copy of the render of the system. Each edit is refused, changing nothing, when it leaves the scene with a problem it did not have | `particles DIR SYSTEM shape N JSON`, `control-point pN [--at X,Y] [--angle D] [--cursor] [--remove]`, `remap add JSON`, `remap set N JSON`, `remap remove N`, `cutout START,END,OPACITY\|none [--layer L]`, `orient screen\|velocity\|fixed\|none [--angle D] [--layer L]`, `perspective [--focal F] [--depth MIN,MAX] [--depth-speed MIN,MAX] [--off]`, `emit-from-layer N LAYER [--threshold T] [--color] [--inherit] [--jitter J] [--refresh R]`, `inherit N color\|size\|alpha\|velocity... [--follow]`, `shader ID\|none [--layer L]`, `collider add JSON`, `collider set N JSON`, `collider remove N`, `collider response N bounce\|slide\|stop\|stick\|kill [--bounce B] [--friction F]`, `keep distance TARGET [--distance D] [--strength S] [--index N]`, `keep between FROM TO [--strength S] [--index N]`, `keep remove N`, `boids [--radius R] [--separation S] [--alignment A] [--cohesion C] [--max-speed V] [--off]`, `operator remap add JSON`, `operator remap set N JSON`, `operator remap remove N`, `rope [--subdivide N] [--width W] [--uv-scale U] [--uv-scroll V] [--smooth BOOL] [--layer L] [--off]`, `trail [--length N] [--width W] [--segments N] [--subdivide N] [--uv-scale U] [--uv-scroll V] [--textured BOOL] [--layer L] [--off]`, `texture ID\|none [--layer L]`, `turbulent [--forward D] [--scale S] [--noise-speed N] [--offset O] [--speed MIN,MAX] [--phase P] [--audio BAND] [--quiet] [--off]`, `offset [--distance D] [--scale S] [--sign X,Y] [--octaves N] [--noise-scale N] [--noise-speed N] [--off]`, `pace [--emitter N] [--max-per-step N]`, `lit [--normal N] [--off] [--layer L]`, `refract [--normal N] [--amount A] [--off] [--layer L]` (`--show` on either prints the two-sided preview, the displacement and what each quality level keeps), `create [--from S] [--parent S] [--on M]`, `delete`, `settings [--capacity N] [--seed N] [--prewarm S] [--space local\|world] [--rate R] [--emitter N] [--replace] [--lock V,V\|none]`, `start [--life\|--speed\|--angle\|--size\|--rotation\|--spin\|--frame\|--alpha\|--radial MIN,MAX[,CURVE]] (an end a number, `prop:ID` or `_` to keep it; the curve kept when left out) [--color C,C] [--velocity X,Y:X,Y] [--hsv MIN,MAX [--saturation MIN,MAX] [--value MIN,MAX] [--hue-steps N] \| --palette C,C... [--palette-noise H,S,V] \| --gradient T:C...] [--clear alpha\|velocity\|radial\|hsv\|palette\|gradient]`, `emitter N [--burst COUNT [--every S] [--on beat\|click]] [--periodic ON_MIN,ON_MAX:OFF_MIN,OFF_MAX] [--duration S] [--delay S] [--follow B] [--audio-gate B] [--inherit F] [--clear V]`, `window N A,B,C,D\|none`, `rename NEW`, `force gravity X,Y`, `force drag A`, `force turbulence --strength S --scale C [--speed V] [--time-scale T] [--mask MIN,MAX]`, `force oscillate position\|alpha\|size --freq MIN,MAX --scale MIN,MAX [--phase MIN,MAX]`, `force attractor TARGET --strength S [--radius R] [--falloff F]`, `force noise A`, `force cap S`, `force torque A`, `force spin-drag A`, `force vortex TARGET --strength S [--radius R] [--falloff F] [--inner I] [--outer O] [--ring R,W,P]`, `force orbit TARGET --strength S [--radius R] [--falloff F]` (each with `--index N` to replace one), `force remove N`, `child add SYSTEM --on M [--probability P] [--max N] [--offset X,Y] [--angle D] [--scale S] [--points-from]`, `child set N SYSTEM --on M …`, `child remove N`, `size-over-life T:S...\|none [--layer L]`, `color-over-life T:C...\|none [--layer L]`, `render [--stretch T[,MIN,MAX]] [--frames time\|life\|random\|none] [--fps F] [--mix B] [--brightness B] [--cycles N] [--order O\|none] [--layer L]`; `particles DIR SYSTEM handles [--layer L] [--time T]` prints what the canvas draws for the system and `particles DIR SYSTEM budget [--time T] [--quality Q] [--layer L]` its budget |
| `light` | one of `add` (`light`: `point`, `spot`, `tube` or `directional`, optional `id`, `into` a group and `at`, its position in that group; a new light starts in the middle of the canvas with the defaults of its type: a point light of radius 400 at a height of 120, a spot light of radius 900 at 150 with an intensity of 2, a cone of `[25, 45]` and a tilt of 45°, a tube light of length 400 and radius 240 at 60, a directional light with an intensity of 0.8 and a tilt of 50°; `add_layer` with `kind: light` and the type as `source` does the same), `set` (`layer` and any of `light`, which drops `cone`, `length` and `texture` where the new type has none, `color`, `intensity` 0 to 100, `radius` 1 to 100 000, `z`, `cone` `[inner, outer]` for spot lights, `length` for tube lights, `tilt` −89 to 89, `at`, a point of the canvas the light moves to, and `aim`, a point of the canvas it turns to, both through its parents as the light is placed at the start of the scene), `remove` (`layer`), `solo` (`layer`, or none to end it), `mute` (`layer`, `muted`), `volumetric` (`layer` of a point or spot light, optional `density` 0 to 1, `samples` 4 to 64 and `shafts` 0 to 1, starting from 0.2, 16 and 0, or `off`) and `projection` (`layer` of a spot light and one of `asset`, a picture or video of the scene, `from`, a layer that takes no light and reads none that does, or `off`); solo and mute are kept in `/scene/ext/editor/lights` and only change the preview; `set` also drops `volumetric` from tube and directional lights | `light DIR LAYER add [--type T] [--into G] [--at X,Y]`, `light DIR LAYER set [--type T] [--color C] [--intensity I] [--radius R] [--z Z] [--cone IN,OUT] [--length L] [--tilt D] [--at X,Y] [--aim X,Y]`, `light DIR LAYER remove`, `light DIR LAYER solo [--off]`, `light DIR LAYER mute [--off]`, `light DIR LAYER falloff [--time T]` (the points of the falloff curve), `light DIR LAYER volumetric [--density D] [--samples N] [--shafts S] [--off]` or `--show` (the glow against the distance, the size and samples each quality level keeps and on phones), `light DIR LAYER projection [--asset A \| --layer L] [--off]` or `--show` (the pictures, videos and layers it may project and where the picture lands on the canvas), and `lights DIR [--time T]` prints every light with its handles, its dirty box and the count against each device tier, or with `--cost [--quality Q]` what the lighting costs at each quality level |
| `lighting` | `layer` (an image, video, text or particles layer; a particles layer takes only `strength` and `emissive`), optional `normal`, `env`, `roughness_map` and `emissive_map` (pictures of the scene; an empty id removes one), `roughness`, `reflection` and `strength` (0 to 1), `emissive` (0 to 16) and `off` (removes `lighting`); the first change writes the block. A normal, roughness or glow map that is not `linear` becomes `linear` in the same step, or, when the picture is also drawn in colors, the field names a new asset `<id>_linear` of the same file that is; `particles … lit` and `refract` do the same with their normal maps | `lighting DIR LAYER [--normal ID] [--env ID] [--roughness-map ID] [--emissive-map ID] [--roughness R] [--reflection R] [--emissive E] [--strength S] [--off]` |
| `normals` | `asset` (a picture) or `layer` (an image layer, whose picture is used), optional `strength` (0 to 16, 2 by default), `blur` (0 to 32 pixels, 1 by default) and `id` (refused when it names an asset that is not a normal map made here); makes a normal map from the edges of the brightness of the picture (a Sobel filter after a Gaussian blur, the same as `wprs-depth` offers), saves it as `normals/<id>-<n>.png` (`.ktx2` with the codec and quality of the picture when that one is KTX2), declares it as a `linear` asset and, with `layer`, sets it as the `normal` of that layer | `normals DIR ASSET --generate [--strength S] [--blur B] [--layer L] [--id ID]`, or `--preview` to print the relief, the map and a lit sphere without keeping anything |
| `paint_map` | `layer`, `map` (`roughness` or `emissive`), `radius`, optional `hardness` and `value` (the gray, 0 to 1) and `points` (fractions of the layer); paints the stroke into the map the layer has when it is a painted map (a `linear` picture under `maps/`) that nothing else uses, or else into a new map that starts as a copy of the one it had, or the size of its picture (at most 512 on the longer side), white for roughness and black for glow, saves it as `maps/<id>-<n>.png` (`.ktx2` with the codec and quality of the map it replaces, or else of the layer's picture, when that one is KTX2), declares it as a `linear` asset and sets `roughness_map` or `emissive_map` | `paint-map DIR LAYER roughness\|emissive --points X,Y;X,Y;... [--radius R] [--hardness H] [--value V]`, or `--heat` to print the map as a heat view |
| `ambient` | optional `ambient` and `sky` (colors), `clear` (removes both); sets `canvas.ambient` and `canvas.sky` | `ambient DIR [--ambient C] [--sky C] [--clear]`, or `--preview` to print the hemisphere they make |
| `import_script` | `file`, a `.js` module; copies it to `scripts/<name>` and declares or replaces its entry in `scripts` with the `events` it exports or handles with `on` and the `uses` it imports or calls (`spawn` and `order`, `effect`, `console`, `camera` for `scene.camera`, `camera3d`, `parallax`, `bloom`, `background`, `ambient` and `sky`, read as whole names or taken apart with `const { parallax } = scene`, `shared` also for imports of other modules), never its `params`, which are written by hand, refusing scripts that would take the scene past 1 MiB or 64 modules; the author checks what was proposed. With `template` (`empty`, or `media`: a module that handles `mediaStatus`, `mediaPlayback`, `mediaProperties`, `mediaThumbnail` and `mediaTimeline` and exports `nowPlaying`, `accentColor`, `shown` and `progress` for values to follow with `fn`) it writes that module instead, named `file` or, when `file` is empty, `<template>.js` (`<template>-2.js` when taken), refused when the file exists; the assets panel offers it as New script… | `import-script DIR FILE [--template empty\|media]` |
| `import_assets` | `package`, a `.wpak` of kind `assets`, and `parts`, each `layer:ID`, `particles:ID`, `script:PATH` or `effect:ID` (every part when empty); copies the parts with what they name and their files under `assets/<id>-<version>/`, renames ids in use, credits the files with their license, attribution and `source`, and adds the package to `origin.derived_from` (see "Shared parts"); a scene of the first version is refused | `import-assets DIR PACKAGE [--part KIND:ID]…` |
| `asset` | `asset` and `cover` (`current`, `previous` or `none`) with an optional `placeholder` (a still picture: an asset id or a path of the folder; required for a new asset) makes that asset the cover of what the media player plays, or of what it played before, shown over the placeholder (`none` makes it a plain picture again), refused when the asset is animated or the value unknown. With `field` (a pointer of the scene such as `/scene/layers/0/image`) and `cover` `current` or `previous`, it shows that cover in the field in one undoable step: the asset `asset` when it already is a cover of that kind, otherwise any cover asset of that kind, otherwise a new one named `asset` (or `asset_2` and so on when the id is taken) over `placeholder`, so that a plain picture never becomes a cover; the picture picker offers it as Cover of what plays and Previous cover (made from the chosen picture, named `cover` and `previous_cover`) and, while a cover is chosen, Shown while nothing plays to pick its placeholder. Otherwise `asset` (a picture of the scene that is not KTX2 yet; an `animated` GIF or WebP becomes an array of its frames with their mean frame time in `KTXanimData`) and `ktx2: true`, optional `codec` (`uastc`, sharper and the default, or `etc1s`, smaller) and `quality` (1 to 100, 80 by default; kept in the file under the key `wprsQuality` so that tools that rewrite the picture keep it); encodes the picture as a KTX2 texture with Basis Universal and its mipmaps (left out when the asset sets `mipmaps: false`, wrapped when it repeats or mirrors, linear when it is `linear`) at its exact size (a side that is not a multiple of 4 pixels is padded inside the last blocks, and the card draws only the picture), writes `assets/<name>.ktx2` (`<name>-2.ktx2` when another file takes the name) and points the asset at it; undo points it back, and `prune` removes the old file once nothing names it. The assets panel offers it as Convert to KTX2 on each picture, with the codec, a quality slider, the file size before and after and, for each class of graphics card (BC7 on desktops, ASTC on phones, ETC2 on older phones, uncompressed on the rest), the memory it takes there and a crop of the middle of the picture as that card draws it, next to the same crop of the picture now (the graphics card of the app draws the crops of the classes it reads, the processor decodes the others, and each crop says which; `classes[i].drawn` is `gpu`, `cpu` or `plain`); the dialog encodes again on a worker thread a moment after the slider stops | `asset DIR ID [--cover current\|previous\|none] [--placeholder PICTURE] [--field POINTER]`, `asset DIR ID` (the asset, its size and what it takes on each class of card), `asset DIR ID --ktx2 [--codec uastc\|etc1s] [--quality N]`, or with `--dry-run [--looks DIR]` to only report and write the crops |
| `rgb` | `layer` (an adjustment layer) and one or more of `on` (`true` sends what it captures to RGB lighting and sets `permissions.rgb` in the same step, `false` stops and drops the permission when no other layer sends), `size` (`[w, h]`, 1 pixel to twice the canvas: the region around the layer position, which is set to the middle of the canvas when the layer has none) and `whole` (drops `size`: the whole canvas); undo returns both. The inspector of an adjustment layer has a switch for it, Whole canvas and A strip of it, and the strip the lights get, read from the preview 10 times a second through `Rgb1.GetStrip` and drawn as a grid of up to 64 × 16 cells; on the preview the region has corner handles that resize it around the layer position as one gesture, and the layer handles move it | `rgb DIR LAYER [--on true\|false] [--size WxH \| --whole]` |
| `translate` | `pointer` to a text of the manifest, `language`, `text` (an empty or missing one removes that language), `add` (adds a language the text lacks, starting from the text in the default language, and leaves one it has alone) | `translate DIR POINTER LANGUAGE TEXT`, `--remove` or `--add` |
| `thumbnail` | `file`, `animated`; copies a picture into `assets/` as `previews.thumbnail` or `previews.animated` (`preview` in a first-version manifest) | `thumbnail DIR FILE [--animated]` |
| `upgrade` | optional `rating`, `reasons`, `photosensitive`, `derived_from`; moves a first-version manifest to `wallpaper-rs.wpak/2` | `upgrade DIR [--rating R] [--reason R] [--photosensitive B] [--derived-from A]` |
| `add_property` | `kind` (`slider`, `bool`, `color`, `combo`, `text`, `file`, `folder`, `button`, `group`, `label` or `divider`), optional `id` (`<kind>_<n>` when omitted), `group`, `label`; the property starts valid, last of its group, and a combo with the options `option_1` and `option_2` and empty labels, so no text in a language the package may not use is written | `add-property DIR KIND` |
| `move_property` | `id`, `to` (its position among the properties of its group, 0 first), optional `group` (an empty one takes it out of its group); the `order` of that group becomes 1, 2… | `move-property DIR ID TO` |
| `rename_property` | `id`, `to`; the bindings, `prop:` triggers, groups, conditions (maps and expressions) and preset values that name it follow; scripts are not rewritten, and the command warns about each script that names the old id in quotes | `rename-property DIR ID TO` |
| `remove_property` | `id`; removes the property, its values in presets and in conditions that are maps, and the `group` of its members; refused, naming each place, while a binding, a `prop:` trigger or an expression condition names it; the command warns about scripts as `rename_property` does | `remove-property DIR ID` |
| `condition` | `id`, `text`, `enabled`: sets `visible_if`, or `enabled_if`, to the expression or, when `text` is a JSON object, to that map; refuses a condition that does not check, and an empty one removes it | `condition DIR ID TEXT [--enabled]` |
| `add_preset` | optional `id` (`preset_<n>` when omitted), `label`; a preset without values | `add-preset DIR` |
| `animate` | one of: `layer` (`layer`: a mesh or a model; `add` (a clip: a new animation layer playing it) or `index` (0 first), then optional `clip`, `weight` 0 to 1, `rate` 0 to 16, `additive`, `blend_in` and `blend_out` 0 to 10 s, `enabled`, `bones` (the bones it moves; empty for all), `to` (a new place), `remove`; at most 16); `root` (`layer`: a model with clips, `on`: sets `root_motion`); `preset` (`layer`, `bone`, `preset`: `hair`, `tail`, `cloth`, `antenna`, `chain` or `jelly`, which set `mode`, `stiffness`, `damping`, `inertia`, `gravity` and `wind` of its body, adding one when it has none); `morph` (`layer`, `shape` of the file or an expression, `weight` −1 to 1 or `null` to drop it) | `animate DIR LAYER layer [--add CLIP \| --index N] [--clip C] [--weight W] [--rate R] [--additive B] [--blend-in S] [--blend-out S] [--enabled B] [--bones A,B] [--to N] [--remove]`, `animate DIR LAYER root true\|false`, `animate DIR LAYER preset BONE PRESET`, `animate DIR LAYER morph SHAPE WEIGHT\|none`; `animate DIR LAYER clips` and `bones` print the clips of the layer and of the file with their events and root path, its animation layers, bones and blend shapes |
| `scene3d` | one of: `model` with `add` (`asset`, optional `id`, `into`, `at`: a position), `set` (`layer`, optional `position`, `rotation`, `scale`: three numbers each, the scale never 0, `size`, `env`: a picture, empty removes it, `asset`) or `remove` (`layer`); `group` (optional `id`, `into`, `layers` to gather; makes a group with `space: 3d`); `space` (`layer`, `deep`); `depth_test` (`layer`, `on`; only flat layers inside a 3D group); `material` (`layer`, `name` of a material of the file, optional `shader`: `pbr`, `unlit` or a package effect, `lit`, `double_sided`, `cast_shadows`, `receive_shadows`, `color`, `emissive` (`null` removes them), `params` with `metallic`, `roughness` and `occlusion` 0 to 1 and `normal_scale` 0 to 10, `clear`); `camera` (optional `fov` 1 to 179, `near`, `far`, `eye`, `center`, `up`, `roll` −180 to 180 degrees around the view, `from_view`, `clear`); `view` (the authoring view in `ext.editor.view3d`: `orbit` in degrees, `pan` in pixels, `zoom` as a factor, `frame` (a model layer), `through`, `reset`); `tones` (optional `hdr`, `tonemap`: `none`, `aces` or `agx`, only with `hdr`, `split` 0 to 1 for the before and after preview); `shadows` (`layer`: a point, spot or directional light, optional `on`, `resolution` 256 to 4096, `automatic` (drops it), `bias` 0 to 1; at most 2 lights); `path` with `add` (`id`: a path of two keys, at 0 and 2 s, where the authoring view looks), `key` (`path`, `time`, optional `eye`, `center`, `up`, `fov` 1 to 179, `from_view`, `from` (moves that key), `remove`; a path keeps 2 keys), `remove` (`path`), `set` (`path`, optional `loop`, `ease`, `to`: a new id), `order` (`sequential` or `random`) or `fade` (optional `to`: `black` or `cross`, `duration` 0 to 10, `clear`); `new_material` (`name`: writes `effects/<name>.json` and `.wgsl` from a template of a material shader and adds the effect); `debug` (optional `show`: `mask`, `matte` or `shadows` in `ext.editor.debug3d`, which the preview draws instead of the light; without it the preview lights again) | `model DIR add\|set\|remove\|info …`, `shadows DIR LAYER [--on B] [--resolution N\|--automatic] [--bias B] [--show]`, `camera-path DIR add\|key\|remove\|set\|order\|fade\|show …`, `new-material DIR NAME [--check]`, `debug3d DIR mask\|matte\|shadows\|none`, `group3d DIR add\|space\|depth-test\|show …`, `material DIR LAYER [NAME] … [--key COLOR] [--list]`, `camera3d DIR …`, `view3d DIR …`, `tonemap DIR …`, or `scene3d DIR --model JSON` (one of `--model`, `--group`, `--space`, `--depth-test`, `--material`, `--camera`, `--view`, `--tones`, `--shadows`, `--path`, `--new-material`, `--debug`) as the app sends it; `model-info FILE.glb` and `cost3d DIR` print what the inspector shows |
| `effects` | one of `add` (`layer`, `effect`: a built-in effect or one of the package, optional `name`, `at` (the top when omitted), `params` and `optional`; starts the stack of a layer that has none), `set` (`layer`, `index`, optional `params`, where `null` goes back to the default, `enabled`, `name`, an empty one removes it, and `optional`), `remove` (`layer`, `index`; the last one removes the stack), `move` (`layer`, `index`, `to`) or `mask` (`layer`, `index`, optional `texture`, `from` (a layer) or `shape`, one of them, which replaces the source of the mask, `channel`, `invert`, `feather`, `strength`, or `off`); refuses an effect it does not know, a parameter the effect lacks, a number outside the range of the parameter, a sound or light layer, a mask without a source and any problem the change adds to the scene, as the inspector does | `effects DIR LAYER add EFFECT [--param K=V] [--name N] [--at I] [--optional]`, `set I [--param K=V] [--unset K] [--enabled B] [--name N] [--optional B]`, `remove I`, `move I TO`, `mask I [--texture A] [--from L] [--shape JSON] [--channel C] [--invert B] [--feather PX] [--strength S] [--off]` |
| `layer` | one of `transform` (`layer`, optional `position`, `rotation` (−360 to 360), `scale`, `skew`, `origin` and `size`, a box of an image, video, solid, adjustment or spectrum layer above 0; what is not given stays, and a given field that follows keyframes or another source is refused unless `replace` writes the plain value over it), `blend` (`layer`, `blend`: a blend mode) or `instance` (`layer`, a particles layer, `values`: `alpha`, `size`, `count`, `speed`, `life`, `time_scale`, `brightness`, `color` or `points`, `null` drawing that value as the system does again, and `clear`); refuses what the scene would not take | `layer DIR LAYER transform [--position X,Y] [--rotation D] [--scale X,Y] [--skew X,Y] [--origin X,Y] [--size W,H] [--replace]`, `blend MODE`, `instance [--alpha A] [--size S] [--count C] [--speed S] [--life L] [--time-scale T] [--brightness B] [--color C] [--point X,Y]... [--unset V] [--clear]` |
| `manifest` | one of `origin` (`kind`, optional `derived_from`, `provider`, `source_url`; replaces the origin), `license` (`license`, an SPDX expression of at most 256 characters), `tags` (`tags`, with `add` or `remove` instead of replacing them), `authors` (`authors`, at most 64; an empty list removes them) or `display` (`values` of `fit`, `zoom`, `position`, `background`, `span`, `flip` and `accent`, and `clear`, settings to remove); refuses what the checks of the manifest refuse, each broken field on its own, so an earlier problem does not hide a new one | `manifest DIR origin KIND [--derived-from AUTHOR\|JSON] [--provider NAME --item-url U --credit C [--terms-url U]] [--source-url U]`, `license L`, `tags T... [--add\|--remove]`, `authors A...`, `display [--fit F] [--zoom Z] [--position X,Y] [--background C] [--span S] [--flip F] [--accent C\|prop:ID] [--clear S]` |
| `property` | `id`, optional `min`, `max` and `step` (sliders only), `default` and `order` (its place among the properties of its group, as `move_property` does); refuses a default outside the range, as validation does | `property DIR ID [--min N] [--max N] [--step N] [--default V] [--order N]` |
| `canvas` | one of `bloom` (optional `threshold`, `strength`, `radius`, `tint`, `hdr` (`[threshold, strength]` of the glare above white, or `none` to remove it; anything else is refused) or `off`; a new glow starts from the defaults) or `parallax` (optional `amount`, `delay`, `pointer`, `gyro` or `off`); the inspector of the canvas shows both with switches and sliders, which change the scene live while dragged, record one step per drag and write nothing when clicked without moving | `canvas DIR bloom [--threshold T] [--strength S] [--radius R] [--tint C] [--hdr T,S\|none] [--off]`, `parallax [--amount A] [--delay D] [--pointer P] [--gyro B] [--off]` |
| `timeline` | `id`, optional `add` (a new timeline, of 5 s unless a length is given; at most 64), `remove` (every value it drove, keyed or tracked, follows the scene clock again), `to` (a new id; every value it drives, keyed or tracked, follows), `duration` (seconds) or `frames` with `fps` (1 to 240), `wrap`, `loop` (`once`, `repeat` or `pingpong`), `speed` (−100 to 100), `delay` (seconds), `autoplay`, `trigger` (empty removes it); refused when it leaves a problem in the scene. The Timelines menu of the timeline adds, renames and removes them | `timeline DIR ID [--add \| --remove] [--rename NEW] [--duration S \| --frames N --fps F] [--wrap B] [--loop L] [--speed X] [--delay S] [--autoplay B] [--trigger T]` |
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
source (keyframes, a user property, the sound analysis, the pointer or the media player) has
a menu to add or remove a keyframe at the current time, bind it, make it fixed again or edit the
binding itself (timeline, loop, scale, offset, clamp, smoothing). Follow the media player lists
only the `media` values that fit the field (the texts of the track for a text, the numbers and
`playing` for a number, `playing` for a switch, the colors of the cover for a color), each with
what it would show now with the simulated player of the Player tab. An optional field that is set
can go back to its default.

The Player tab is a simulated media player for the preview, so that a scene can be made without
music playing: a switch turns it on, and it has the title, artist and album of a made-up track,
whether it plays, its position and length (sliders up to four hours, with a field in seconds for longer tracks, and times written as `m:ss` or, from one hour, `h:mm:ss` as the scene writes them) and its cover, picked among the pictures of the scene, with the five colors that the scene would
get from it (or from the placeholder) as swatches. Next track gives the made-up track another title
and the next picture as its cover, so that what waits for `media:track` and `media:cover` starts in
the preview, and the playing switch starts what waits for `media:play` or `media:pause`; the track
and its cover stay the same while only the position, length or playing change.
Scripts of the preview get the made-up track too, with their media events, and `wprs-ctl media
simulate OUTPUT FILE.json` shows one on any screen (`dbus.md`, `SimulateMedia`).
The preview then shows that track instead of what a media player plays, without asking for the
`media` grant and without reading the players; it reaches only the preview.

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
The inspector of a light picks its type and color, slides its intensity, radius, height, length,
cone and, for spot and directional lights, its tilt toward the canvas (also a handle on the aim line,
nearer the light the more it faces the canvas), plots its light against the distance with the curve component and toggles solo and mute.

Selecting the canvas lists every light with its type and solo and mute toggles, how many lights the
scene has against the 12 it may have and how many each device tier keeps (8 on T0 and T1), the
pickers of the light around and from above with a sphere lit by them, and the preview quality picker
of the particle budget, which here shows for each level whether the lighting is worked out once (Q0)
or how many pixel lights it shades each frame while a light moves.

The inspector of an image, video or text layer has a Lighting part (a particles layer has its switch
and the glow and strength sliders): a switch that writes or removes
`lighting`, pickers of the normal map, the reflected picture and the roughness and glow maps, sliders
for roughness, reflection, glow and strength, a note when a map is read as colors, Generate normals (an image layer), a dialog with
sliders for strength and blur that shows the relief of the map lit from the top left, the map itself
and a lit sphere before it keeps the map, and Paint roughness and Paint glow, which paint on the layer
in the preview with a brush size, hardness and gray while the map shows over it as a heat view.
`wprs-ctl scene lights`, `light … falloff`, `normals --preview`, `paint-map --heat` and
`ambient --preview` print the same data.

A point or spot light has a Light the air part: a switch for `volumetric`, sliders for density,
samples and shafts, the glow plotted against the distance, and for each quality level whether the air is off
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

### Models and 3D

The Add layer menu offers 3D model (once the scene has a `.glb` asset), 3D group and 3D model from a
file, a dialog that reads a binary glTF file before it is imported and tells its triangles,
materials, textures, bones and clips, and for each device tier whether it fits, the largest texture
it gets (2048 px on phones and on T0 and T1) and the texture memory, with a note on converting FBX and
OBJ with Blender; accepting imports the file and adds a model layer, inside the selected group.

A scene with models draws over the preview an authoring view with its own camera, kept in
`ext.editor.view3d`, which only the preview reads: dragging with the right button turns it around its
target, with the middle button or Shift slides it and the wheel brings it closer, and the preview
renders through it while the scene camera shows as a dashed frustum with a handle on its eye, a square
on the point it looks at, both moved in the plane of the view, and a line toward its up. Each model
shows its box, which selects it with a click; the selected model shows three arrows along x, y and z
that move it along that direction, a knob above it that turns it (sideways around y, up and down
around x) and a square on its box corner that scales it. These handles start from the values the
model has at the current time and write `position`, `rotation` and `scale`: a keyed value gets a key at
the current time, a value bound to a property or a script keeps its binding and its handle is dimmed,
and `scene3d` `model` `set` refuses to write over keys or a binding.

The inspector of a model layer shows the counts and the tier verdicts of its file, a Reflects picker of
the pictures of the scene (`env`) with a thumbnail, Look at it in the authoring view, and a Materials
part with a row per material of the file: a ball shaded with the material and the light around and from
above of the scene (as in the light tools), the shader picker (physical, unlit or a package effect),
lit, both sides, casts and takes shadows, tint and glow colors, sliders for metal, roughness, relief and
occlusion, and the thumbnails of the maps of the file (color, metal and roughness, normals, occlusion,
glow). The inspector of a group turns it 3D, and the inspector of a flat layer inside a 3D group
toggles Hide behind nearer models (`depth_test`).

Selecting the canvas also shows the 3D camera: sliders for the field of view and the roll that change
the preview while they move, where it looks from and toward, Set the camera from the view, Look
through the camera and Back to the camera view; and Bright colors: a switch for `hdr`, the tone map
picker and a Before and after slider that keeps `ext.editor.tonemap_split`, so the preview shows the
part on the left without the tone map. The quality picker of the lights panel lists for each level the
triangles, the 3D groups, the work units of the models and whether reflections of pictures are drawn,
and for each tier the quality it starts at, whether the models fit and their texture memory.
`wprs-ctl scene model`, `group3d`, `material`, `camera3d`, `view3d`, `tonemap`, `model-info` and
`cost3d` do the same from the terminal, and `group3d … show` prints the handles the preview draws.

The inspector of a model with a skeleton or clips adds an Animation part: its clips with their length and
animation, On the timeline (the clip timeline of meshes, whose rows for a model key `angles` and `shift`
of the chosen bone and the weights of its blend shapes, and which shows the events of the clip as marks
that seek to them), New clip from the file with the animations of the file and their length, Root motion,
Swinging bones (a bone picker and the presets, with the bodies the layer has), a slider per blend shape
for its weight, and Animation layers: per layer the clip it plays, On, Adds, move up, remove, sliders
for weight, speed, fade in and fade out, and a bone mask picker (Only these bones); the same Animation
layers part shows under the bones of a mesh. In the authoring view the selected model draws its bones
(a line from each bone to its parent and a knob that picks the bone for the timeline and the pickers)
and, with root motion, the path its first clip walks over two turns. The picked bone shows a ring: dragging
the ring turns the bone around the view axis, dragging inside it tips the bone like a trackball, and a
square in its middle moves it in the plane of the view; each drag keys `angles` or `shift` of that bone
in the chosen clip at the playhead (a model without clips gets `clip_1`). A swinging bone draws the fan
its rotation bound allows in its own plane, with Least and Most knobs that drag the bound, and under
Swinging bones each body shows its wind slider, the bounds of its turn and slide and Show the wind on
the preview, which draws the arrow of the wind of the scene. `rigs::query` answers `bone3d` (`layer`,
`bone`, `clip`, `time`, `clip_time`, `from` with the values the drag started at, and `turn` in degrees,
`tilt` `[x, y]` in degrees or `slide` `[x, y]` in pixels) with the `angles` and `shift` to key, and
`bound3d` (`layer`, `bone`, `point`) with the degrees of the bound under a point;
`wprs-ctl scene key … --angles X,Y,Z --shift X,Y,Z` and `scene physics … add BONE --rotation MIN,MAX`
do the same from the terminal.

A material row picks Fur, Foliage or Chroma key besides Physical and Unlit and the material shaders of
the package: fur shows sliders for shells, length and density, the shells each quality level draws and
a comb pad (dragged across for x and y, a slider toward the viewer); foliage shows sway and speed, the
wind of the canvas with its arrow, and Show the sway mask of the vertex colors; chroma shows a key color
picker, tolerance and feather sliders and Show the matte. A shader of the package shows a warning that it
runs on the graphics card within the limits of package effects. Under the materials, New material shader
writes a template, and every material shader of the package shows whether it builds, or its error with
the line and column of its file.

The lights panel shows Shadows: per point, spot or directional light a switch, the size of its map
(automatic or 256 to 4 096) with the size each tier draws, on phones and at Q0, and a bias slider, and
Show where shadows fall, which draws lit places white and shadowed ones red in the preview and the
dashed frusta of the maps in the authoring view. The 3D camera panel shows Camera paths: per path its
length, Loop, the easing, remove, its keys (a click removes one) and Key the view at the current time,
Add from the view, the order, the fade with its length, and Fly through the paths from the camera, which
looks through the scene camera and plays from the start. In the authoring view each path draws its
curve, a knob on each key that moves the eye of that key and a square on the point it looks at, both
moved in the plane of the view at their own depth, a dashed line between them, and a square where the
paths put the camera at the current time. The scene timeline shows a lane per path with its keys, which
drag to another time (`key` with `from`, `camera-path … key PATH T --from T` in the terminal) and seek
the preview with a click. The cost lines add the shadow maps, the fur shells and
the skinned vertices of each quality level and the shadowed lights of each tier. `wprs-ctl scene
animate`, `clip`, `key`, `material`, `shadows`, `camera-path`, `new-material`, `debug3d` and `cost3d`
do the same from the terminal.

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
started again, and `--stop` ends it; `--media FILE` shows a made-up track from a JSON file of at
most 64 KiB (`title`, `artist`, `album`, `album_artist`, `subtitle`, `genres`, the last three
words or lists of words, `playing`, `position` and `duration` in seconds from 0 to 10 000 000 and
`rate` from −100 to 100, `cover`, a picture inside the folder; texts of at most 512 characters
without control characters) instead of a
media player. The document of a preview may carry `preview: { quality, restart, media }`, which the
daemon reads only for previews; `media` is refused, naming the field, when a value is out of
range, and never reads the media players or needs the `media` grant.

The canvas draws the outline of each layer with handles to turn and scale the selected one, and
guides for 16:9, 21:9, 32:9, 4:3, 16:10 and 9:16 screens. *Screens* previews the scene spread over
several screens: two or three side by side, two stacked, or the user's own layout as the daemon
reports it (`GetState().outputs[x].mm`). It frames the part of the canvas that each screen shows
and darkens what the frames between them (`layout.bezel_mm`) and the edges of the layout hide, and
its *Spread over every screen* box sets or removes `display.span: "all-outputs"` in the manifest,
as an undoable step. The parts come from the editor query `{"spread": {"layout", "bezel",
"monitors"}}` (`rigs::query`), which answers `{spread, canvas, screens, bounds, parts: [{id, rect}]}`
with each rect in canvas pixels, or `error` for an unknown layout or screens without a name or a
size (at most 64). `wprs-ctl scene preview DIR --layout dual|triple|vertical|mine --out FILE
[--time S]` writes the same view as a picture, without a desktop: the scene at its poster time, or
at second `S` of the scene clock when `--time` is 0 or more (as the editor shows the moment its
preview is at), each screen's part at its physical place and the frames between them. Dragging a layer that is not locked
selects it and moves it, as one gesture. A drag that starts outside the layers, or any drag with
Shift, draws a rectangle that selects the layers it touches and picks them as Ctrl does in the
layer list, ready for *Group*. Ctrl and the wheel zoom, the wheel or a drag with the middle or
right button pans.

## Shared parts

The *Assets* panel of the scene editor has two tools for `assets` packages (`package.md`, "Shared
resources"). *Make an assets package* lists the layers, particle systems, scripts and package effects
of the scene with the selected layer checked; each checked part takes a title and a thumbnail, and the
package takes a title, an id, a license, the name its parts are credited to and a preview picture.
It saves the scene, writes a new folder with `assets.json`, the files the parts use, the credits of
the scene that cover them and the pictures made into jpegs of at most 1024 pixels and 1 MiB, and
offers to open that folder in the editor, where it is built like a scene. `wprs-ctl pack new-assets
SCENE OUT --part KIND:ID… --id ID --title TITLE --license SPDX --attribution NAME [--thumbnail FILE]
[--part-thumbnail KIND:ID=FILE] [--part-title KIND:ID=TITLE]` does the same.

*Import from an assets package* opens a `.wpak`, shows each part with its thumbnail and, before
anything is copied, the license and credit of every file it brings; the checked parts are imported
with `import_assets`, one step of the history. `wprs-ctl pack parts FILE [--json]` lists the same.

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
