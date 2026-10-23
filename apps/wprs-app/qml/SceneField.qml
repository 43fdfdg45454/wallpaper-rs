import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: field

    required property var daemon
    required property var modelData
    readonly property string kind: modelData.kind
    readonly property string text: String(modelData.value ?? "")

    signal edited(string key, var value)

    function hex(c) {
        const parts = modelData.alpha ? [c.r, c.g, c.b, c.a] : [c.r, c.g, c.b];
        return "#" + parts.map(v => Math.round(v * 255).toString(16).padStart(2, "0")).join("");
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: field.modelData.label + (field.kind === "slider" ? ": " + Math.round(slider.value * 1000) / 1000 + (field.modelData.unit ? " " + field.modelData.unit : "") : "")
        textFormat: Text.PlainText
        font.bold: field.kind === "group"
        wrapMode: Text.Wrap
    }

    QQC2.Switch {
        visible: field.kind === "bool"
        Accessible.name: field.modelData.label
        checked: field.modelData.value === true
        onToggled: field.edited(field.modelData.key, checked)
    }

    QQC2.Slider {
        id: slider

        Layout.fillWidth: true
        visible: field.kind === "slider"
        from: field.modelData.min ?? 0
        to: field.modelData.max ?? 1
        stepSize: field.modelData.step ?? 0
        snapMode: QQC2.Slider.SnapAlways
        value: Number(field.modelData.value ?? 0)
        onMoved: settle.restart()

        Timer {
            id: settle

            interval: 150
            onTriggered: field.edited(field.modelData.key, Math.round(slider.value * 10000) / 10000)
        }
    }

    QQC2.Button {
        id: swatch

        visible: field.kind === "color"
        text: field.text
        icon.name: "color-picker"
        icon.color: field.text.length === 9 ? "#" + field.text.slice(7) + field.text.slice(1, 7) : field.text || "transparent"
        onClicked: colors.open()

        ColorDialog {
            id: colors

            selectedColor: swatch.icon.color
            options: field.modelData.alpha ? ColorDialog.ShowAlphaChannel : 0
            onAccepted: field.edited(field.modelData.key, field.hex(selectedColor))
        }
    }

    QQC2.ComboBox {
        visible: field.kind === "combo"
        model: field.modelData.options
        textRole: "label"
        valueRole: "value"
        currentIndex: field.modelData.options.findIndex(option => option.value === field.modelData.value)
        onActivated: field.edited(field.modelData.key, currentValue)
    }

    QQC2.TextField {
        Layout.fillWidth: true
        visible: field.kind === "text" && !field.modelData.multiline
        text: field.text
        maximumLength: field.modelData.max_len ?? 1024
        onEditingFinished: {
            if (text !== field.text)
                field.edited(field.modelData.key, text);
        }
    }

    QQC2.TextArea {
        Layout.fillWidth: true
        visible: field.kind === "text" && field.modelData.multiline
        text: field.text
        onEditingFinished: {
            if (text !== field.text)
                field.edited(field.modelData.key, text);
        }
    }

    RowLayout {
        visible: field.kind === "file"

        QQC2.Button {
            text: field.text.split("/").pop() || field.daemon.tr("Choose…")
            icon.name: "document-open"
            onClicked: files.open()
        }

        QQC2.ToolButton {
            visible: field.text !== ""
            icon.name: "edit-clear"
            text: field.daemon.tr("Clear")
            display: QQC2.AbstractButton.IconOnly
            onClicked: field.edited(field.modelData.key, null)
        }

        FileDialog {
            id: files

            nameFilters: [field.daemon.tr("Images") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp)"]
            onAccepted: field.edited(field.modelData.key, field.daemon.path(selectedFile))
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: field.modelData.help !== ""
        text: field.modelData.help
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        color: Kirigami.Theme.disabledTextColor
    }
}
