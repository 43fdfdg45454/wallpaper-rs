import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: cast

    required property var tools
    property var info: null
    readonly property var entries: info ? [{
            "label": App.tr("Nothing"),
            "off": true
        }].concat(info.pictures.map(id => ({
                "label": id,
                "asset": id
            }))).concat(info.videos.map(id => ({
                "label": App.tr("%1 (video)", id),
                "asset": id
            }))).concat(info.layers.map(id => ({
                "label": App.tr("%1 (layer)", id),
                "from": id
            }))) : []
    readonly property int current: {
        const now = info?.current;
        if (typeof now === "string")
            return entries.findIndex(e => e.asset === now);
        if (now?.layer)
            return entries.findIndex(e => e.from === now.layer);
        return 0;
    }

    function load() {
        info = tools.light?.type === "spot" ? JSON.parse(tools.page.editor.rig(tools.lamp, JSON.stringify({
            "projection": {
                "layer": tools.lamp,
                "time": tools.page.editor.time
            }
        })) || "null") : null;
    }

    visible: info !== null
    Component.onCompleted: load()

    Connections {
        function onLightChanged() {
            cast.load();
        }

        target: cast.tools
    }

    QQC2.Label {
        text: App.tr("Projects")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: cast.entries.map(e => e.label)
        currentIndex: Math.max(0, cast.current)
        onActivated: index => {
            const e = cast.entries[index];
            cast.tools.page.run("light", {
                "projection": Object.assign({
                    "layer": cast.tools.lamp
                }, e.off ? {
                    "off": true
                } : e.asset ? {
                    "asset": e.asset
                } : {
                    "from": e.from
                })
            });
        }
    }

    Image {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 6
        Layout.preferredHeight: Layout.preferredWidth
        visible: !!cast.info?.image
        source: cast.info?.image ?? ""
        fillMode: Image.PreserveAspectFit
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: cast.current > 0
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: cast.info?.quad ? App.tr("The cone on the preview shows where the picture lands.") : App.tr("The beam does not reach the canvas; tilt it toward the canvas to see where the picture lands.")
    }
}
