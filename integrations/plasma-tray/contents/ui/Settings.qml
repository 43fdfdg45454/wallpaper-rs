import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Repeater {
    id: form

    required property var daemon
    property var values: ({})

    signal edited(string key, string value)

    function shown(field) {
        const value = values[field.key];
        return Array.isArray(value) ? value.join(",") : String(value ?? "");
    }

    function hint(field) {
        if (field.kind !== "interval" || typeof field.default !== "number")
            return String(field.default ?? "");
        const units = {
            "d": 86400,
            "h": 3600,
            "m": 60,
            "s": 1
        };
        let rest = field.default, text = "";
        for (const unit in units) {
            if (rest >= units[unit])
                text += Math.floor(rest / units[unit]) + unit;
            rest %= units[unit];
        }
        return text || "0s";
    }

    delegate: RowLayout {
        id: row

        required property var modelData
        readonly property string text: form.shown(modelData)

        Kirigami.FormData.label: modelData.kind === "bool" ? "" : form.daemon.field(modelData.key) + ":"

        QQC2.CheckBox {
            visible: row.modelData.kind === "bool"
            text: form.daemon.field(row.modelData.key)
            checked: row.text === "true"
            onToggled: form.edited(row.modelData.key, String(checked))
        }

        QQC2.SpinBox {
            id: number

            readonly property real shown: Number(row.text || row.modelData.default || 0)

            visible: row.modelData.kind === "int"
            enabled: !word.checked
            from: row.modelData.min ?? -2147483647
            to: row.modelData.max ?? 2147483647
            stepSize: row.modelData.step ?? 1
            editable: true
            value: isNaN(shown) ? 60 : shown
            onValueModified: form.edited(row.modelData.key, String(value))
        }

        QQC2.CheckBox {
            id: word

            visible: row.modelData.kind === "int" && row.modelData.values.length > 0
            text: form.daemon.value(row.modelData.values[0] ?? "")
            checked: row.modelData.values.includes(row.text)
            onToggled: form.edited(row.modelData.key, checked ? row.modelData.values[0] : String(number.value))
        }

        QQC2.ComboBox {
            visible: row.modelData.kind === "enum"
            model: row.modelData.values.map(value => ({
                        "value": value,
                        "text": form.daemon.value(value)
                    }))
            textRole: "text"
            valueRole: "value"
            currentIndex: Math.max(0, row.modelData.values.indexOf(row.text || row.modelData.default))
            onActivated: form.edited(row.modelData.key, currentValue)
        }

        QQC2.Button {
            visible: row.modelData.kind === "color"
            text: row.text || form.daemon.tr("Choose…")
            icon.name: "color-picker"
            onClicked: colors.open()

            ColorDialog {
                id: colors

                selectedColor: row.text || "black"
                onAccepted: form.edited(row.modelData.key, selectedColor.toString())
            }
        }

        QQC2.TextField {
            visible: !["bool", "int", "enum", "color"].includes(row.modelData.kind)
            text: row.text
            placeholderText: form.hint(row.modelData)
            Layout.preferredWidth: Kirigami.Units.gridUnit * 14
            onEditingFinished: {
                if (text !== row.text)
                    form.edited(row.modelData.key, text);
            }
        }

        QQC2.ToolButton {
            visible: row.modelData.clearable && row.text !== ""
            icon.name: "edit-clear"
            text: form.daemon.tr("Clear")
            display: QQC2.AbstractButton.IconOnly
            onClicked: form.edited(row.modelData.key, "")
        }
    }
}
