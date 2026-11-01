import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: field

    required property var daemon
    required property string output
    required property var modelData
    property bool folded: false
    readonly property string kind: modelData.kind
    readonly property string text: String(modelData.value ?? "")
    readonly property bool log: modelData.log === true
    readonly property real low: modelData.min ?? 0
    readonly property real high: modelData.max ?? 1
    readonly property int places: modelData.decimals ?? 0

    signal edited(string key, var value)
    signal fold(string key, bool on)

    function hex(c) {
        const parts = modelData.alpha ? [c.r, c.g, c.b, c.a] : [c.r, c.g, c.b];
        return "#" + parts.map(v => Math.round(v * 255).toString(16).padStart(2, "0")).join("");
    }

    function toValue(p) {
        return log ? low * Math.pow(high / low, p) : p;
    }

    function toPosition(v) {
        return log ? Math.log(Math.max(v, low) / low) / Math.log(high / low) : v;
    }

    function shown(v) {
        return Number(v).toLocaleString(Qt.locale(), "f", places) + (modelData.unit ? " " + modelData.unit : "");
    }

    enabled: modelData.enabled !== false

    Kirigami.Separator {
        Layout.fillWidth: true
        visible: field.kind === "divider" || field.kind === "group"
    }

    RowLayout {
        visible: field.kind !== "button" && field.modelData.label !== ""

        QQC2.ToolButton {
            visible: field.kind === "group"
            icon.name: field.folded ? "go-next" : "go-down"
            display: QQC2.AbstractButton.IconOnly
            text: field.folded ? field.daemon.tr("Show") : field.daemon.tr("Hide")
            onClicked: field.fold(field.modelData.key, !field.folded)
        }

        Kirigami.Icon {
            visible: field.modelData.icon !== "" && field.modelData.icon !== undefined
            source: field.modelData.icon ?? ""
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: implicitWidth
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: field.modelData.label + (field.kind === "slider" ? ": " + field.shown(field.toValue(slider.value)) : "")
            textFormat: Text.PlainText
            font.bold: field.kind === "group"
            font.italic: field.kind === "divider"
            wrapMode: Text.Wrap
        }
    }

    QQC2.Switch {
        visible: field.kind === "bool"
        Accessible.name: field.modelData.label
        checked: field.modelData.value === true
        onToggled: field.edited(field.modelData.key, checked)
    }

    RowLayout {
        Layout.fillWidth: true
        visible: field.kind === "slider"

        QQC2.Slider {
            id: slider

            Layout.fillWidth: true
            Accessible.name: field.modelData.label
            from: field.log ? 0 : field.low
            to: field.log ? 1 : field.high
            stepSize: field.log ? 0 : (field.modelData.step ?? 0)
            snapMode: QQC2.Slider.SnapAlways
            value: field.toPosition(Number(field.modelData.value ?? field.low))
            onMoved: settle.restart()

            Timer {
                id: settle

                interval: 150
                onTriggered: field.edited(field.modelData.key, field.toValue(slider.value))
            }
        }

        QQC2.TextField {
            visible: field.modelData.editable === true
            Accessible.name: field.modelData.label
            implicitWidth: Kirigami.Units.gridUnit * 5
            text: Number(field.modelData.value ?? field.low).toLocaleString(Qt.locale(), "f", field.places)
            onEditingFinished: {
                const typed = Number.fromLocaleString(Qt.locale(), text);
                if (!isNaN(typed))
                    field.edited(field.modelData.key, typed);
            }
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

    SceneChoices {
        visible: field.kind === "combo"
        modelData: field.modelData
        onEdited: (key, value) => field.edited(key, value)
    }

    QQC2.Button {
        visible: field.kind === "button"
        text: field.modelData.label
        icon.name: field.modelData.icon || "media-playback-start"
        onClicked: field.daemon.act("PressSceneButton", [field.output, field.modelData.key])
    }

    SceneEntry {
        Layout.fillWidth: true
        daemon: field.daemon
        output: field.output
        modelData: field.modelData
        visible: ["text", "file", "folder"].includes(field.kind)
        onEdited: (key, value) => field.edited(key, value)
    }

    SceneShortcut {
        Layout.fillWidth: true
        daemon: field.daemon
        modelData: field.modelData
        visible: field.kind === "shortcut"
        onEdited: (key, value) => field.edited(key, value)
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
