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
    property string mask
    property var brush: null
    readonly property var edit: JSON.parse(editor.json || "{}")
    readonly property var values: JSON.parse(editor.values || "{}")
    readonly property var ops: edit.ops ?? []
    readonly property var tools: [[App.tr("Crop and rotate"), "transform-crop"], [App.tr("Screens"), "video-display"], [App.tr("Light and color"), "adjustlevels"], [App.tr("Curves"), "adjustcurves"], [App.tr("Effects"), "draw-brush"], [App.tr("Masks"), "edit-select"], [App.tr("Depth"), "object-order-back"], [App.tr("Parallax"), "transform-move"]]

    function run(action, args) {
        editor.run(action, JSON.stringify(args ?? {}));
    }

    function exercise() {
        editor.preview(false);
        editor.run("set", '{"changes": ["ev=0.3", "vibrance=12"]}');
        editor.run("crop", '{"aspect": "16:9", "rotate": 90}');
        editor.run("crop", JSON.stringify({
            "output": App.outputs[0] ?? "DP-1"
        }));
        editor.run("curve", '{"points": ["0,0", "0.5,0.6", "1,1"]}');
        editor.run("overlay", '{"mask": "radial:0.5,0.5,0.3,0.3", "opacity": 0.5}');
        editor.run("mask", '{"name": "sky", "spec": "luminance:0,0.4"}');
        editor.run("depth", '{"near": 0.9, "dilate": 4}');
        editor.run("parallax", '{"source": "pointer", "steps": 8}');
        editor.undo();
        editor.redo();
        editor.compare(true);
        editor.compare(false);
        if (!editor.canUndo || editor.canRedo)
            throw new Error("the image editor did not record its steps");
        for (let i = page.tools.length - 1; i >= 0; i--)
            side.currentIndex = i;
        editor.preview(true);
    }

    title: App.tr("Edit %1", item.title || String(item.path ?? "").split("/").pop())
    padding: 0
    Component.onCompleted: editor.open(item.path ?? "", item.width ?? 0, item.height ?? 0)
    actions: bar.actions

    Editor {
        id: editor
    }

    EditActions {
        id: bar

        page: page
    }

    header: Kirigami.InlineMessage {
        type: editor.error !== "" ? Kirigami.MessageType.Error : Kirigami.MessageType.Positive
        text: editor.error || (editor.exported ? App.tr("Saved as %1", editor.exported) : "")
        visible: text !== ""
    }

    RowLayout {
        anchors.fill: parent
        enabled: editor.ready
        spacing: 0

        EditSide {
            id: side

            Layout.preferredWidth: Kirigami.Units.gridUnit * 13
            Layout.maximumWidth: Layout.preferredWidth
            Layout.fillWidth: false
            Layout.fillHeight: true
            page: page
        }

        Kirigami.Separator {
            Layout.fillHeight: true
        }

        ColumnLayout {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 17
            Layout.maximumWidth: Layout.preferredWidth
            Layout.fillWidth: false
            Layout.fillHeight: true

            Kirigami.Heading {
                Layout.margins: Kirigami.Units.smallSpacing
                level: 3
                text: (page.tools[side.currentIndex] ?? [""])[0]
            }

            QQC2.ScrollView {
                id: panel

                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth

                StackLayout {
                    x: Kirigami.Units.smallSpacing
                    width: panel.availableWidth - 2 * Kirigami.Units.smallSpacing
                    currentIndex: side.currentIndex

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
            section: side.currentIndex
        }
    }
}
