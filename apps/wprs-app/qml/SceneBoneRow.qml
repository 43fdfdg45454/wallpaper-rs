import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Rectangle {
    id: row

    required property var tree
    required property var bone
    readonly property var page: tree.host.page
    readonly property bool chosen: page.mesh.bone === bone.name
    property bool renaming: false

    implicitHeight: line.implicitHeight
    color: chosen ? Kirigami.Theme.highlightColor : drop.containsDrag ? Kirigami.Theme.hoverColor : "transparent"
    DropArea {
        id: drop

        anchors.fill: parent
        keys: ["bone"]
        onDropped: drop => row.tree.hang(drop.source.bone.name, row.bone.name)
    }

    QQC2.Label {
        id: ghost

        z: 3
        visible: Drag.active
        text: row.bone.name
        Drag.keys: ["bone"]
        Drag.source: row
        Drag.hotSpot: Qt.point(width / 2, height / 2)
    }

    RowLayout {
        id: line

        anchors.fill: parent
        anchors.leftMargin: row.bone.depth * Kirigami.Units.gridUnit

        Kirigami.Icon {
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            source: "handle-sort"
            visible: row.tree.host.editable

            DragHandler {
                target: ghost
                onActiveChanged: {
                    if (active) {
                        ghost.Drag.start();
                        return;
                    }
                    ghost.Drag.drop();
                    ghost.x = 0;
                    ghost.y = 0;
                }
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: !row.renaming
            text: row.bone.name
            color: row.chosen ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
            elide: Text.ElideRight

            TapHandler {
                onTapped: row.page.mesh.bone = row.bone.name
                onDoubleTapped: row.renaming = row.tree.host.editable
            }
        }

        QQC2.TextField {
            Layout.fillWidth: true
            visible: row.renaming
            text: row.bone.name
            onVisibleChanged: if (visible)
                forceActiveFocus()
            onEditingFinished: {
                if (row.renaming && text.trim() !== "" && text.trim() !== row.bone.name)
                    row.tree.host.bone({
                        "name": row.bone.name,
                        "to": text.trim()
                    });
                row.renaming = false;
            }
        }

        QQC2.ToolButton {
            icon.name: "edit-rename"
            text: App.tr("Rename")
            display: QQC2.AbstractButton.IconOnly
            visible: row.tree.host.editable && !row.renaming
            onClicked: row.renaming = true
        }

        QQC2.ToolButton {
            icon.name: "edit-delete"
            text: App.tr("Remove")
            display: QQC2.AbstractButton.IconOnly
            visible: row.tree.host.editable
            onClicked: row.tree.host.bone({
                "name": row.bone.name,
                "remove": true
            })
        }
    }
}
