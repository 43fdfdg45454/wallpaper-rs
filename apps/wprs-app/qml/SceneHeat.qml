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
    readonly property var colors: shown ? (mesh.view === "owner" ? shown.owner : shown.heat) : []

    function load() {
        if (!visible)
            return;
        shown = mesh.ask(host.layerId, {
            "weights": mesh.bone || null
        });
        paper.requestPaint();
    }

    onVisibleChanged: load()
    Component.onCompleted: load()

    Connections {
        function onBoneChanged() {
            heat.load();
        }

        function onViewChanged() {
            paper.requestPaint();
        }

        target: heat.mesh
    }

    Connections {
        function onViewChanged() {
            heat.load();
        }

        target: heat.host.gizmo.page.editor
    }

    Canvas {
        id: paper

        anchors.fill: parent
        opacity: 0.75
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            const s = heat.shown;
            if (!s)
                return;
            s.triangles.forEach((t, i) => {
                const a = s.points[t[0]], b = s.points[t[1]], c = s.points[t[2]];
                ctx.fillStyle = heat.colors[i];
                ctx.beginPath();
                ctx.moveTo(a[0] * width, a[1] * height);
                ctx.lineTo(b[0] * width, b[1] * height);
                ctx.lineTo(c[0] * width, c[1] * height);
                ctx.closePath();
                ctx.fill();
            });
        }
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
