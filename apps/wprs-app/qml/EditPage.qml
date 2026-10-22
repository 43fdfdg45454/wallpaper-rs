import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Page {
    id: page

    required property var item
    property alias editor: editor
    property string aspect
    readonly property var edit: JSON.parse(editor.json || "{}")
    readonly property var values: JSON.parse(editor.values || "{}")
    readonly property var ops: edit.ops ?? []
    readonly property var tools: [App.tr("Crop and rotate"), App.tr("Screens"), App.tr("Light and color"), App.tr("Curves"), App.tr("Effects"), App.tr("Masks"), App.tr("Depth"), App.tr("Parallax")]

    function run(action, args) {
        editor.run(action, JSON.stringify(args ?? {}));
    }

    function exercise() {
        editor.run("set", '{"changes": ["ev=0.3", "vibrance=12"]}');
        editor.run("crop", '{"aspect": "16:9", "rotate": 90}');
        editor.run("crop", JSON.stringify({"output": App.outputs[0] ?? "DP-1"}));
        editor.run("curve", '{"points": ["0,0", "0.5,0.6", "1,1"]}');
        editor.run("overlay", '{"mask": "radial:0.5,0.5,0.3,0.3", "opacity": 0.5}');
        editor.run("mask", '{"name": "sky", "spec": "luminance:0,0.4"}');
        editor.run("depth", '{"near": 0.9, "dilate": 4}');
        editor.run("parallax", '{"source": "pointer", "steps": 8}');
        for (let i = page.tools.length - 1; i >= 0; i--)
            pick.currentIndex = i;
        editor.preview(true);
    }

    title: App.tr("Edit %1").arg(item.title || String(item.path ?? "").split("/").pop())
    padding: 0
    Component.onCompleted: editor.open(item.path ?? "", item.width ?? 0, item.height ?? 0)
    actions: [
        Kirigami.Action {
            text: App.tr("Back")
            icon.name: "go-previous"
            onTriggered: App.navigate(0, "")
        },
        Kirigami.Action {
            text: App.tr("Save")
            icon.name: "document-save"
            enabled: editor.dirty
            onTriggered: editor.save()
        },
        Kirigami.Action {
            text: App.tr("Show on the desktop")
            icon.name: "view-visible"
            checkable: true
            checked: editor.live
            enabled: editor.ready
            onTriggered: editor.preview(!checked)
        },
        Kirigami.Action {
            text: App.tr("Reset all")
            icon.name: "edit-reset"
            enabled: editor.ready
            onTriggered: page.run("reset")
        }
    ]

    Editor {
        id: editor
    }

    header: Kirigami.InlineMessage {
        type: Kirigami.MessageType.Error
        text: editor.error
        visible: text !== ""
    }

    RowLayout {
        anchors.fill: parent
        enabled: editor.ready

        ColumnLayout {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 17
            Layout.fillHeight: true

            QQC2.ComboBox {
                id: pick

                Layout.fillWidth: true
                model: page.tools
            }

            QQC2.ScrollView {
                id: panel

                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth

                StackLayout {
                    width: panel.availableWidth
                    currentIndex: pick.currentIndex

                    EditGeometry {
                        page: page
                    }

                    EditScreens {
                        page: page
                    }

                    EditLight {
                        page: page
                    }

                    EditCurves {
                        page: page
                    }

                    EditEffects {
                        page: page
                    }

                    EditMasks {
                        page: page
                    }

                    EditDepth {
                        page: page
                    }

                    EditParallax {
                        page: page
                    }
                }
            }
        }

        EditCanvas {
            Layout.fillWidth: true
            Layout.fillHeight: true
            page: page
            screens: pick.currentIndex === 1
        }

        QQC2.ScrollView {
            id: side

            Layout.preferredWidth: Kirigami.Units.gridUnit * 15
            Layout.fillHeight: true
            contentWidth: availableWidth

            EditOps {
                width: side.availableWidth
                page: page
            }
        }
    }
}
