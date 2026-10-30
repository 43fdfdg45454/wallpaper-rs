pragma Singleton
import QtQuick

QtObject {
    property var own: ({})
    readonly property var words: Object.assign({}, early, SceneTerms.words, SceneNames.words, SceneLabels.words)
    readonly property var early: ({
            "a": App.tr("Alpha"),
            "aberration": App.tr("Color fringes"),
            "accept": App.tr("Accepted files"),
            "additive": App.tr("Add on top"),
            "align": App.tr("Alignment"),
            "alignment": App.tr("Alignment"),
            "alpha": App.tr("Transparency"),
            "ambient": App.tr("Ambient light"),
            "amount": App.tr("Amount"),
            "amplitude": App.tr("Amplitude"),
            "anchor": App.tr("Anchor"),
            "angle": App.tr("Angle"),
            "angle2": App.tr("Second angle"),
            "animated": App.tr("Animated"),
            "animation": App.tr("Animation"),
            "animations": App.tr("Animation layers"),
            "arc": App.tr("Arc"),
            "around": App.tr("Around a point"),
            "aspect": App.tr("Aspect"),
            "assets": App.tr("Assets"),
            "assets.wrap": App.tr("Texture edges"),
            "at": App.tr("Control point"),
            "atlas": App.tr("Sprite grid"),
            "attach": App.tr("Attach to"),
            "attack": App.tr("Attack"),
            "attractor": App.tr("Attractor"),
            "audio": App.tr("Sound"),
            "audio_band": App.tr("Sound band"),
            "audio_gate": App.tr("Only while sound plays"),
            "autoplay": App.tr("Play at start"),
            "b": App.tr("Blue"),
            "background": App.tr("Background color"),
            "bands": App.tr("Bands"),
            "base": App.tr("Base value"),
            "beat": App.tr("Beat"),
            "bend": App.tr("Bend"),
            "between": App.tr("Between two points"),
            "bias": App.tr("Bias"),
            "blend": App.tr("Blend mode"),
            "blend_in": App.tr("Blend in"),
            "blend_out": App.tr("Blend out"),
            "blend_space": App.tr("Blend space"),
            "bloom": App.tr("Glow"),
            "blur": App.tr("Blur"),
            "boids": App.tr("Flocking"),
            "bone": App.tr("Bone"),
            "bones": App.tr("Bones"),
            "bool": App.tr("Switch"),
            "bottom_left": App.tr("Bottom left"),
            "bottom_right": App.tr("Bottom right"),
            "bounce": App.tr("Bounce"),
            "bounds": App.tr("Bounds"),
            "box": App.tr("Box"),
            "brightness": App.tr("Brightness"),
            "burst": App.tr("Burst"),
            "bursts": App.tr("Bursts"),
            "button": App.tr("Button"),
            "cache": App.tr("Cache"),
            "camera": App.tr("Camera"),
            "camera3d": App.tr("3D camera"),
            "canvas": App.tr("Canvas"),
            "cap": App.tr("Speed limit"),
            "capacity": App.tr("Capacity"),
            "cast_shadows": App.tr("Casts shadows"),
            "center": App.tr("Center point"),
            "chain": App.tr("Bone chain"),
            "channel": App.tr("Channel"),
            "children": App.tr("Child systems"),
            "circle": App.tr("Circle"),
            "clamp": App.tr("Clamp to"),
            "clamp_in": App.tr("Clamp input"),
            "clamp_out": App.tr("Clamp output"),
            "clear": App.tr("Clear each frame"),
            "click": App.tr("Click"),
            "clip": App.tr("Clip"),
            "clip_to": App.tr("Clip to part"),
            "clips": App.tr("Clips"),
            "clock": App.tr("Clock"),
            "cohesion": App.tr("Cohesion"),
            "collapsed": App.tr("Collapsed"),
            "collide": App.tr("Collisions"),
            "color": App.tr("Color"),
            "color_a": App.tr("First color"),
            "color_b": App.tr("Second color"),
            "color_over_life": App.tr("Color over life"),
            "colors": App.tr("Colors"),
            "combo": App.tr("Choice"),
            "cone": App.tr("Cone"),
            "contrast": App.tr("Contrast"),
            "count": App.tr("Count"),
            "coverage": App.tr("Coverage"),
            "cull": App.tr("Hidden faces"),
            "cursor": App.tr("Pointer"),
            "curvature": App.tr("Curvature"),
            "cutout": App.tr("Cutout"),
            "cycles": App.tr("Cycles per life"),
            "damping": App.tr("Damping"),
            "date": App.tr("Date"),
            "decay": App.tr("Decay"),
            "decimals": App.tr("Decimals"),
            "default": App.tr("Default"),
            "default_asset": App.tr("Default asset"),
            "delay": App.tr("Delay"),
            "density": App.tr("Density"),
            "depth": App.tr("Depth"),
            "depth_speed": App.tr("Depth speed"),
            "depth_test": App.tr("Depth test"),
            "depth_write": App.tr("Write depth"),
            "descriptor": App.tr("Descriptor"),
            "direction": App.tr("Direction"),
            "distance": App.tr("Distance"),
            "dither": App.tr("Dithering"),
            "divider": App.tr("Divider"),
            "double": App.tr("Double"),
            "double_sided": App.tr("Two sided"),
            "down": App.tr("Falling"),
            "drag": App.tr("Drag"),
            "duration": App.tr("Duration"),
            "durations": App.tr("Frame durations"),
            "dye_decay": App.tr("Color fading"),
            "ease": App.tr("Easing"),
            "edge": App.tr("Edge"),
            "editable": App.tr("Number box"),
            "effect": App.tr("Effect"),
            "effects": App.tr("Effects"),
            "ellipse": App.tr("Ellipse"),
            "ellipsis": App.tr("Ellipsis"),
            "emissive": App.tr("Own light"),
            "emitters": App.tr("Emitters"),
            "enabled": App.tr("Enabled"),
            "enabled_if": App.tr("Enabled when"),
            "end": App.tr("End"),
            "env": App.tr("Reflected picture"),
            "events": App.tr("Events"),
            "every": App.tr("Every"),
            "exponent": App.tr("Exponent"),
            "exposure": App.tr("Exposure"),
            "expressions": App.tr("Expressions"),
            "ext": App.tr("Extensions"),
            "eye": App.tr("Eye")
        })

    function label(key, pointer) {
        const text = String(key ?? "");
        const path = String(pointer ?? "").split("/");
        const books = path[1] === "manifest" ? [ManifestWords.words, words] : [words, ManifestWords.words];
        const near = path.slice(-3, -1).reverse().map(part => part + "." + text).concat(text);
        const named = near.map(word => books.map(book => book[word]).find(found => found)).find(found => found);
        return named ?? (text.charAt(0).toUpperCase() + text.slice(1)).replace(/_/g, " ");
    }

    function kind(key) {
        return SceneValues.kinds[key] ?? label(key);
    }

    function effect(id) {
        return own[id] ?? SceneEffects.effects[id] ?? label(id);
    }

    function easing(name) {
        const parts = /^(in_out|in|out)_(.*)$/.exec(name);
        if (!parts)
            return SceneValues.values[name] ?? label(name);
        const curve = SceneValues.curves[parts[2]] ?? parts[2];
        return parts[1] === "in" ? App.tr("Ease in (%1)", curve) : parts[1] === "out" ? App.tr("Ease out (%1)", curve) : App.tr("Ease in and out (%1)", curve);
    }

    function value(option) {
        const text = String(option ?? "");
        return SceneValues.values[text] ?? ManifestWords.values[text] ?? SceneValues.kinds[text] ?? own[text] ?? SceneEffects.effects[text] ?? (words[text] ? label(text) : easing(text));
    }

    function binding(bound) {
        const found = Object.keys(SceneValues.sources).find(source => (bound ?? {})[source] !== undefined) ?? "keys";
        const detail = found === "prop" ? bound.prop : ["audio", "cursor"].includes(found) ? value(bound[found]) : "";
        return SceneValues.sources[found] + (detail ? " · " + detail : "");
    }
}
