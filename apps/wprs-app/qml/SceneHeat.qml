import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: heat

    required property var host
    readonly property var mesh: host.mesh
    property var shown: null
    readonly property string picture: shown ? (mesh.view === "owner" ? shown.owner : shown.heat) : ""

    function load() {
        if (!visible)
            return;
        shown = mesh.ask(host.layerId, {
            "weights": mesh.bone || null
        });
    }

    onVisibleChanged: load()
    Component.onCompleted: load()

    Connections {
        function onBoneChanged() {
            heat.load();
        }

        target: heat.mesh
    }

    Connections {
        function onViewChanged() {
            heat.load();
        }

        target: heat.host.gizmo.page.editor
    }

    Image {
        anchors.fill: parent
        opacity: 0.75
        smooth: true
        cache: false
        source: heat.picture
    }

    Flow {
        anchors.top: parent.bottom
        anchors.left: parent.left
        width: Math.max(parent.width, Kirigami.Units.gridUnit * 12)
        spacing: Kirigami.Units.smallSpacing
        visible: heat.mesh.view === "owner" && heat.shown !== null

        Repeater {
            model: heat.shown ? heat.shown.bones : []

            delegate: RowLayout {
                required property var modelData
                required property int index

                Rectangle {
                    implicitWidth: Kirigami.Units.gridUnit * 0.7
                    implicitHeight: implicitWidth
                    color: heat.shown.colors[index]
                }

                QQC2.Label {
                    text: modelData
                }
            }
        }
    }
}
