import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: inspector

    required property var page
    readonly property string pointer: page.editor.selected
    readonly property var parts: pointer.split("/").slice(1)

    RowLayout {
        Layout.fillWidth: true

        QQC2.ToolButton {
            icon.name: "go-previous"
            text: App.tr("Up")
            display: QQC2.AbstractButton.IconOnly
            enabled: inspector.parts.length > 1
            onClicked: inspector.page.editor.select("/" + inspector.parts.slice(0, -1).join("/"))
        }

        Kirigami.Heading {
            Layout.fillWidth: true
            level: 4
            elide: Text.ElideLeft
            text: inspector.parts.map((part, i) => SceneWords.label(part, "/" + inspector.parts.slice(0, i + 1).join("/"))).join(" › ")
        }

        QQC2.ToolButton {
            visible: /\/\d+$/.test(inspector.pointer)
            icon.name: "edit-delete"
            text: App.tr("Remove")
            display: QQC2.AbstractButton.IconOnly
            onClicked: {
                const at = inspector.pointer;
                inspector.page.editor.select("/" + inspector.parts.slice(0, -2).join("/"));
                inspector.page.run("remove", {
                    "pointer": at
                });
            }
        }
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: parent.width

            Repeater {
                model: inspector.page.fields

                delegate: SceneInput {
                    required property var modelData

                    Layout.fillWidth: true
                    page: inspector.page
                    field: modelData
                }
            }
        }
    }
}
