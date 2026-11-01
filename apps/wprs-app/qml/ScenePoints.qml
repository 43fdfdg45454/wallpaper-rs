import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: spots

    required property var host
    readonly property var mesh: host.mesh
    readonly property var list: visible ? (host.rig.points ?? []).filter(p => !p.part && p.at) : []

    Repeater {
        model: spots.list

        delegate: Rectangle {
            id: spot

            required property var modelData

            x: modelData.at[0] * spots.width - width / 2
            y: modelData.at[1] * spots.height - height / 2
            width: Kirigami.Units.gridUnit * 0.8
            height: width
            radius: 2
            color: spots.mesh.spot === modelData.name ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.neutralTextColor
            border.color: Kirigami.Theme.backgroundColor

            QQC2.Label {
                anchors.left: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: spot.modelData.name
            }

            TapHandler {
                onTapped: spots.mesh.spot = spot.modelData.name
            }

            DragHandler {
                onActiveChanged: {
                    if (active)
                        return;
                    spots.mesh.spot = spot.modelData.name;
                    spots.host.gizmo.page.run("point", {
                        "layer": spots.host.layerId,
                        "name": spot.modelData.name,
                        "at": spots.host.at(spot.mapToItem(spots.host, spot.width / 2, spot.height / 2))
                    });
                }
            }
        }
    }
}
