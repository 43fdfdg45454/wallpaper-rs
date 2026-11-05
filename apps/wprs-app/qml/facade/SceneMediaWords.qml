pragma Singleton
import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    readonly property var words: [["title", App.tr("Title")], ["artist", App.tr("Artist")], ["album", App.tr("Album")], ["album_artist", App.tr("Album artist")], ["subtitle", App.tr("Subtitle")], ["genres", App.tr("Genres")], ["kind", App.tr("Kind")]]
    readonly property var numbers: [["position", App.tr("Position")], ["duration", App.tr("Duration")], ["progress", App.tr("Progress")]]
    readonly property var colors: [["primary", App.tr("Primary color of the cover")], ["secondary", App.tr("Secondary color of the cover")], ["tertiary", App.tr("Tertiary color of the cover")], ["text", App.tr("Text color readable over the primary")], ["contrast", App.tr("Black or white over the primary")]]
    readonly property var playing: [["playing", App.tr("Playing")]]

    function clock(seconds) {
        const s = Math.max(0, Math.floor(seconds));
        const m = Math.floor(s / 60);
        return m + ":" + String(s % 60).padStart(2, "0");
    }

    function sample(key, text) {
        const m = SceneLook.media ?? SceneLook.sample;
        const at = m.position ?? 0;
        const length = m.duration ?? 0;
        const progress = length > 0 ? Math.min(1, at / length) : 0;
        switch (key) {
        case "playing":
            return m.playing ? App.tr("yes") : App.tr("no");
        case "position":
            return text ? clock(at) : String(Math.round(at));
        case "duration":
            return text ? clock(length) : String(Math.round(length));
        case "progress":
            return text ? Math.floor(progress * 100) + "%" : progress.toFixed(2);
        case "kind":
            return "music";
        default:
            return Array.isArray(m[key]) ? m[key].join(", ") : String(m[key] ?? "");
        }
    }

    function sources(widget) {
        const text = ["text", "texts", "markup"].includes(widget);
        const found = text ? words : widget === "color" ? colors : ["bool", "number", "integer", "scalar"].includes(widget) ? playing : [];
        return found.map(s => [s[0], s[1], widget === "color" ? "" : sample(s[0], text)]);
    }
}
