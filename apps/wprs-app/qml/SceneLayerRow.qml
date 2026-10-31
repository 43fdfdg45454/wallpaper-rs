import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

QQC2.ItemDelegate {
    id: row

    required property var owner
    required property var modelData

    width: ListView.view.width
    highlighted: modelData.pointer === row.owner.page.editor.selected || row.owner.page.picked.includes(modelData.id)
    leftPadding: Kirigami.Units.largeSpacing * (1 + modelData.depth * 2)
    onClicked: row.owner.pick(modelData, false)

    MouseArea {
        anchors.fill: parent
        z: -1
        onClicked: mouse => row.owner.pick(row.modelData, (mouse.modifiers & Qt.ControlModifier) !== 0)
    }

    function under(point) {
        const view = row.ListView.view;
        const at = view.contentItem.mapFromItem(null, point.x, point.y);
        return row.owner.shown[view.indexAt(at.x, at.y)] ?? null;
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        height: 2
        visible: row.owner.target === row.modelData.pointer
        color: Kirigami.Theme.highlightColor
    }

    contentItem: RowLayout {
        Kirigami.Icon {
            source: "handle-sort"
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
            opacity: grip.active ? 1 : 0.5
            Accessible.name: App.tr("Drag to move the layer")

            HoverHandler {
                cursorShape: Qt.SizeVerCursor
            }

            DragHandler {
                id: grip

                target: null
                xAxis.enabled: false
                onCentroidChanged: {
                    if (active)
                        row.owner.target = row.under(centroid.scenePosition)?.pointer ?? "";
                }
                onActiveChanged: {
                    if (!active)
                        row.owner.drop(row.modelData, row.owner.shown.find(layer => layer.pointer === row.owner.target));
                }
            }
        }

        Kirigami.Icon {
            source: row.owner.icon(row.modelData.type)
            Layout.preferredWidth: Kirigami.Units.iconSizes.small
            Layout.preferredHeight: Kirigami.Units.iconSizes.small
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: row.modelData.label ?? row.modelData.id
            textFormat: Text.PlainText
            elide: Text.ElideRight
            opacity: row.modelData.hidden ? 0.5 : 1
        }

        Repeater {
            model: [["hide", "hidden", row.modelData.hidden, "view-hidden", App.tr("Hide")], ["lock", "locked", row.modelData.locked, "object-locked", App.tr("Lock")], ["solo", "", row.modelData.solo, "favorite", App.tr("Solo")]]

            delegate: QQC2.ToolButton {
                required property var modelData

                icon.name: modelData[3]
                text: modelData[4]
                display: QQC2.AbstractButton.IconOnly
                checkable: true
                checked: modelData[2]
                opacity: checked ? 1 : 0.4
                onClicked: {
                    const args = {
                        "id": row.modelData.id
                    };
                    if (modelData[1])
                        args[modelData[1]] = checked;
                    row.owner.page.run(modelData[0], modelData[0] === "solo" && !checked ? {} : args);
                }
            }
        }
    }
}
