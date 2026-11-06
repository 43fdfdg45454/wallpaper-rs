import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Flow {
    id: styles

    required property var host
    readonly property real chosen: Number(host.value ?? host.field.default ?? 0)

    function draw(ctx, w, h, style, phase) {
        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = "#303030";
        ctx.fillRect(0, 0, w, h);
        ctx.fillStyle = "#ffffff";
        ctx.strokeStyle = "#ffffff";
        const at = phase * 1.4 - 0.2;
        if (style === 0 || style === 1) {
            ctx.fillRect(w * (at - 0.06), 0, w * 0.12, h);
            if (style === 1) {
                ctx.globalAlpha = 0.6;
                ctx.fillRect(w * (at - 0.3), 0, w * 0.06, h);
                ctx.globalAlpha = 1;
            }
        } else if (style === 2) {
            const r = Math.max(0.05, phase) * 0.45;
            ctx.globalAlpha = Math.min(1, 2 - phase * 1.6);
            ctx.lineWidth = h * 0.12;
            ctx.beginPath();
            ctx.ellipse(w * (0.5 - r), h * (0.5 - r), w * r * 2, h * r * 2);
            ctx.stroke();
            ctx.globalAlpha = 1;
        } else {
            const s = 0.1 + 0.25 * Math.sin(phase * Math.PI);
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            ctx.moveTo(w * (0.5 - s), h * 0.5);
            ctx.lineTo(w * (0.5 + s), h * 0.5);
            ctx.moveTo(w * 0.5, h * (0.5 - s));
            ctx.lineTo(w * 0.5, h * (0.5 + s));
            ctx.stroke();
            ctx.beginPath();
            ctx.arc(w * 0.5, h * 0.5, h * s * 0.3, 0, Math.PI * 2);
            ctx.fill();
        }
    }

    Layout.fillWidth: true
    spacing: Kirigami.Units.smallSpacing

    Repeater {
        model: styles.host.field.options

        delegate: QQC2.ToolButton {
            id: choice

            required property var modelData
            required property int index

            checkable: true
            checked: Number(modelData) === styles.chosen
            display: QQC2.AbstractButton.TextUnderIcon
            text: styles.host.field.names[index] ?? String(modelData)
            Accessible.name: text
            onClicked: styles.host.commit(Number(modelData))

            QQC2.ToolTip {
                visible: choice.hovered
                delay: Kirigami.Units.toolTipDelay

                contentItem: ColumnLayout {
                    Canvas {
                        id: moving

                        property real phase: 0

                        Layout.alignment: Qt.AlignHCenter
                        implicitWidth: Kirigami.Units.gridUnit * 9
                        implicitHeight: Kirigami.Units.gridUnit * 6
                        onPhaseChanged: requestPaint()
                        onPaint: styles.draw(getContext("2d"), width, height, Number(choice.modelData), phase)

                        NumberAnimation on phase {
                            from: 0
                            to: 1
                            duration: 1600
                            loops: Animation.Infinite
                            running: choice.hovered
                        }
                    }

                    QQC2.Label {
                        Layout.alignment: Qt.AlignHCenter
                        text: choice.text
                        textFormat: Text.PlainText
                    }
                }
            }

            contentItem: ColumnLayout {
                Canvas {
                    Layout.alignment: Qt.AlignHCenter
                    implicitWidth: Kirigami.Units.gridUnit * 3
                    implicitHeight: Kirigami.Units.gridUnit * 2
                    onPaint: styles.draw(getContext("2d"), width, height, Number(choice.modelData), 0.5)
                }

                QQC2.Label {
                    Layout.alignment: Qt.AlignHCenter
                    text: choice.text
                    font.bold: choice.checked
                    textFormat: Text.PlainText
                }
            }
        }
    }
}
