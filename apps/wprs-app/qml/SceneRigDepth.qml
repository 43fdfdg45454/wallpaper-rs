import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: sculpt

    required property var page
    required property string layerId
    required property bool editable
    readonly property bool painting: page.mesh.sculpt !== null && page.mesh.rigging === layerId
    property var depth: null

    function load() {
        depth = JSON.parse(page.editor.value(page.mesh.pointer(layerId) + "/depth") || "null");
    }

    function change(args) {
        page.run("parts", {
            "layer": layerId,
            "depth": args
        });
    }

    function brush() {
        if (painting)
            page.mesh.sculpt = {
                "by": raise.value,
                "radius": Math.round(size.value * 1000) / 1000
            };
    }

    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            sculpt.load();
        }

        target: sculpt.page.editor
    }

    QQC2.CheckBox {
        text: App.tr("Depth: nearer vertices cover farther ones")
        checked: sculpt.depth !== null
        onToggled: sculpt.change(checked ? {
            "scale": 1
        } : {
            "off": true
        })
    }

    RowLayout {
        visible: sculpt.depth !== null

        QQC2.Label {
            text: App.tr("Perspective of the depth")
        }

        QQC2.Slider {
            Layout.fillWidth: true
            from: 0
            to: 10
            stepSize: 0.1
            value: sculpt.depth?.scale ?? 1
            onMoved: sculpt.change({
                "scale": Math.round(value * 10) / 10
            })
        }
    }

    QQC2.CheckBox {
        visible: sculpt.depth !== null
        text: App.tr("Hide faces seen from behind")
        checked: sculpt.depth?.cull === true
        onToggled: sculpt.change({
            "cull": checked
        })
    }

    QQC2.CheckBox {
        objectName: "sorted"
        visible: sculpt.depth !== null
        text: App.tr("Draw the parts from back to front, so soft edges show what lies behind them")
        checked: sculpt.depth?.sort !== false
        onToggled: sculpt.change({
            "sort": checked
        })
    }

    QQC2.Button {
        visible: sculpt.editable
        text: App.tr("Paint depth on the preview")
        icon.name: "draw-brush"
        checkable: true
        checked: sculpt.painting
        onToggled: sculpt.page.mesh.tool(sculpt.layerId, "depth", checked ? {
            "by": raise.value,
            "radius": Math.round(size.value * 1000) / 1000
        } : null)
    }

    RowLayout {
        visible: sculpt.painting

        QQC2.Label {
            text: App.tr("Pixels towards the viewer")
        }

        QQC2.SpinBox {
            id: raise

            from: -1000
            to: 1000
            stepSize: 5
            value: 20
            editable: true
            onValueModified: sculpt.brush()
        }

        QQC2.Label {
            text: App.tr("Brush size")
        }

        QQC2.Slider {
            id: size

            Layout.fillWidth: true
            from: 0.01
            to: 0.5
            value: 0.1
            onMoved: sculpt.brush()
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: sculpt.painting
        wrapMode: Text.Wrap
        text: App.tr("Strokes on the mesh raise its vertices towards the viewer, or lower them below zero; with depth on, nearer vertices cover farther ones.")
    }
}
