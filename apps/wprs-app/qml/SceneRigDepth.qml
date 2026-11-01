import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: sculpt

    required property var page
    required property string layerId
    required property bool editable
    readonly property bool painting: page.sculpt !== null && page.rigging === layerId

    function brush() {
        if (painting)
            page.sculpt = {
                "by": raise.value,
                "radius": Math.round(size.value * 1000) / 1000
            };
    }

    visible: editable

    QQC2.Button {
        text: App.tr("Paint depth on the preview")
        icon.name: "draw-brush"
        checkable: true
        checked: sculpt.painting
        onToggled: {
            sculpt.page.rigging = checked ? sculpt.layerId : "";
            sculpt.page.sculpt = checked ? {
                "by": raise.value,
                "radius": Math.round(size.value * 1000) / 1000
            } : null;
        }
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
