import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Flow {
    id: styles

    required property var host
    readonly property real chosen: Number(host.value ?? host.field.default ?? 0)

    function draw(ctx, w, h, style) {
        ctx.clearRect(0, 0, w, h);
        ctx.fillStyle = "#303030";
        ctx.fillRect(0, 0, w, h);
        ctx.fillStyle = "#ffffff";
        ctx.strokeStyle = "#ffffff";
        if (style === 0 || style === 1) {
            ctx.fillRect(w * 0.44, 0, w * 0.12, h);
            if (style === 1) {
                ctx.globalAlpha = 0.6;
                ctx.fillRect(w * 0.2, 0, w * 0.06, h);
                ctx.globalAlpha = 1;
            }
        } else if (style === 2) {
            ctx.lineWidth = h * 0.12;
            ctx.beginPath();
            ctx.ellipse(w * 0.15, h * 0.15, w * 0.7, h * 0.7);
            ctx.stroke();
        } else {
            ctx.lineWidth = 1.5;
            ctx.beginPath();
            ctx.moveTo(w * 0.2, h * 0.5);
            ctx.lineTo(w * 0.8, h * 0.5);
            ctx.moveTo(w * 0.5, h * 0.2);
            ctx.lineTo(w * 0.5, h * 0.8);
            ctx.stroke();
            ctx.beginPath();
            ctx.arc(w * 0.5, h * 0.5, h * 0.1, 0, Math.PI * 2);
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
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: styles.host.commit(Number(modelData))

            contentItem: ColumnLayout {
                Canvas {
                    Layout.alignment: Qt.AlignHCenter
                    implicitWidth: Kirigami.Units.gridUnit * 3
                    implicitHeight: Kirigami.Units.gridUnit * 2
                    onPaint: styles.draw(getContext("2d"), width, height, Number(choice.modelData))
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
