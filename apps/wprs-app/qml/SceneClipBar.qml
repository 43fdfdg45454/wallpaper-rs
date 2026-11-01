import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

QQC2.ToolBar {
    id: clipBar

    required property var line
    readonly property var mesh: line.mesh
    readonly property var clips: (JSON.parse(line.page.editor.value(mesh.pointer(mesh.layer) + "/clips") || "null") ?? []).map(c => c.id)
    readonly property var eases: ["step", "linear", "cubic"]

    function clip(args) {
        line.page.run("clip", Object.assign({
            "layer": mesh.layer,
            "id": mesh.clip
        }, args));
    }

    ColumnLayout {
        anchors.fill: parent

        RowLayout {
            QQC2.ComboBox {
                Layout.preferredWidth: implicitWidth
                model: clipBar.clips
                currentIndex: clipBar.clips.indexOf(clipBar.mesh.clip)
                onActivated: index => clipBar.mesh.clip = clipBar.clips[index]
            }

            SceneClipEdit {
                bar: clipBar
            }

            QQC2.Label {
                text: App.tr("Length")
            }

            QQC2.SpinBox {
                from: 1
                to: 36000
                value: Math.round(clipBar.line.length * 10)
                editable: true
                textFromValue: v => (v / 10).toFixed(1)
                valueFromText: text => Math.round(Number(text.replace(",", ".")) * 10)
                onValueModified: clipBar.clip({
                    "length": value / 10
                })
            }

            QQC2.ComboBox {
                readonly property var loops: ["repeat", "pingpong", "once"]

                model: [App.tr("Repeat"), App.tr("Back and forth"), App.tr("Once")]
                currentIndex: Math.max(0, loops.indexOf(clipBar.line.repeat))
                onActivated: index => clipBar.clip({
                        "loop": loops[index]
                    })
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.ToolButton {
                icon.name: "window-close"
                text: App.tr("Back to the timelines")
                display: QQC2.AbstractButton.IconOnly
                onClicked: clipBar.mesh.clip = ""
            }
        }

        RowLayout {
            QQC2.ToolButton {
                icon.name: clipBar.line.page.editor.playing ? "media-playback-pause" : "media-playback-start"
                text: clipBar.line.page.editor.playing ? App.tr("Pause") : App.tr("Play")
                display: QQC2.AbstractButton.IconOnly
                onClicked: clipBar.line.seek(clipBar.line.page.clock, !clipBar.line.page.editor.playing)
            }

            QQC2.Label {
                text: App.tr("%1 s", clipBar.line.now.toFixed(2))
            }

            QQC2.Slider {
                Layout.fillWidth: true
                from: 0
                to: clipBar.line.length
                value: clipBar.line.now
                onMoved: clipBar.line.seek(value, false)
            }

            QQC2.Button {
                text: App.tr("Key")
                icon.name: "keyframe-add"
                enabled: clipBar.mesh.bone !== ""
                onClicked: {
                    clipBar.line.key({
                        "kind": "bone",
                        "name": clipBar.mesh.bone
                    }, Object.assign({
                        "time": clipBar.line.now
                    }, clipBar.mesh.pending(clipBar.mesh.bone)));
                    clipBar.mesh.pose = null;
                }
            }

            QQC2.ComboBox {
                enabled: clipBar.line.picked !== null
                model: [App.tr("Step"), App.tr("Linear"), App.tr("Smooth curve")]
                currentIndex: clipBar.line.picked ? Math.max(0, clipBar.eases.indexOf(clipBar.line.picked.ease)) : 1
                onActivated: index => clipBar.line.key(clipBar.line.chosen.row, Object.assign({
                        "time": clipBar.line.picked.t,
                        "ease": clipBar.eases[index]
                    }, clipBar.line.held(clipBar.line.chosen.row, clipBar.line.picked)))
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                text: App.tr("Remove the key")
                display: QQC2.AbstractButton.IconOnly
                enabled: clipBar.line.picked !== null
                onClicked: {
                    clipBar.line.key(clipBar.line.chosen.row, Object.assign({
                        "time": clipBar.line.picked.t,
                        "remove": true
                    }, clipBar.line.held(clipBar.line.chosen.row, clipBar.line.picked)));
                    clipBar.line.chosen = null;
                }
            }
        }
    }
}
