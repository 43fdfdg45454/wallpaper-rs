import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: windows

    required property var tools
    readonly property var forces: {
        tools.info;
        return JSON.parse(tools.page.editor.value(SceneSwarmKnobs.pointer(tools.system, "/forces")) || "null") ?? [];
    }

    function name(force) {
        return Object.keys(force).find(k => k !== "window") ?? "";
    }

    function put(index, window, gesture) {
        tools.edit("window", window ? {
                "index": index,
                "window": window
            } : {
                "index": index
            }, gesture);
    }

    visible: forces.length > 0

    Kirigami.Heading {
        level: 4
        text: App.tr("When in the life each force acts")
    }

    SceneRepeater {
        id: listed

        rows: windows.forces

        delegate: ColumnLayout {
            readonly property var modelData: listed.rows[index]
            required property int index

            Layout.fillWidth: true

            QQC2.CheckBox {
                text: App.tr("%1: only in part of the life", SceneWords.value(windows.name(modelData)))
                checked: Array.isArray(modelData.window)
                onToggled: windows.put(index, checked ? [0, 0.1, 0.9, 1] : null, false)
            }

            SceneLifeWindow {
                Layout.fillWidth: true
                visible: Array.isArray(modelData.window)
                window: modelData.window ?? null
                onEdited: (w, gesture) => windows.put(index, w, gesture)
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("From left to right the life of a particle; the force fades in, acts in full and fades out where the shape rises, stays and falls")
    }
}
