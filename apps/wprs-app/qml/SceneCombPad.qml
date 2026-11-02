import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

RowLayout {
    id: comb

    required property var comb
    signal combed(string axis, real value)

    QQC2.Label {
        text: App.tr("Comb")
    }

    Rectangle {
        id: pad

        readonly property real side: Kirigami.Units.gridUnit * 5

        Layout.preferredWidth: side
        Layout.preferredHeight: side
        color: Kirigami.Theme.alternateBackgroundColor
        border.color: Kirigami.Theme.disabledTextColor

        Canvas {
            anchors.fill: parent
            onPaint: {
                const ctx = getContext("2d");
                ctx.reset();
                ctx.strokeStyle = Kirigami.Theme.highlightColor;
                ctx.lineWidth = 2;
                ctx.beginPath();
                ctx.moveTo(width / 2, height / 2);
                ctx.lineTo(width / 2 + comb.comb[0] * width / 2, height / 2 - comb.comb[1] * height / 2);
                ctx.stroke();
            }

            Connections {
                function onCombChanged() {
                    parent.requestPaint();
                }

                target: comb
            }
        }

        Rectangle {
            width: Kirigami.Units.gridUnit * 0.7
            height: width
            radius: width / 2
            color: Kirigami.Theme.highlightColor
            x: pad.width / 2 * (1 + comb.comb[0]) - width / 2
            y: pad.height / 2 * (1 - comb.comb[1]) - height / 2
        }

        MouseArea {
            anchors.fill: parent
            onReleased: mouse => {
                const x = Math.max(-1, Math.min(1, mouse.x / pad.width * 2 - 1));
                const y = Math.max(-1, Math.min(1, 1 - mouse.y / pad.height * 2));
                comb.combed("comb_x", x);
                comb.combed("comb_y", y);
            }
        }

        QQC2.ToolTip.visible: hover.hovered
        QQC2.ToolTip.text: App.tr("Drag to comb the fur across the surface")

        HoverHandler {
            id: hover
        }
    }

    ColumnLayout {
        QQC2.Label {
            text: App.tr("Toward the viewer")
            font: Kirigami.Theme.smallFont
        }

        QQC2.Slider {
            from: -1
            to: 1
            value: comb.comb[2]
            onMoved: comb.combed("comb_z", value)
        }
    }
}
