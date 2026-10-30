import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: presets

    required property var daemon
    required property string output
    required property var props
    readonly property var saved: props.user_presets ?? []
    property string notice: ""

    function copy(text) {
        clip.text = text;
        clip.selectAll();
        clip.copy();
        notice = daemon.tr("The settings were copied");
    }

    function paste() {
        clip.text = "";
        clip.paste();
        return clip.text;
    }

    function imported(text) {
        daemon.query("ImportSceneValues", [output, text, ""], "Daemon1", reply => presets && (presets.notice = (reply[0] ?? []).join("\n")), error => presets && (presets.notice = error.message));
    }

    function exported(done) {
        daemon.query("ExportSceneValues", [output, ""], "Daemon1", reply => done(reply[0]), error => presets && (presets.notice = error.message));
    }

    TextEdit {
        id: clip

        visible: false
    }

    RowLayout {
        QQC2.ComboBox {
            id: chooser

            visible: count > 0
            model: (presets.props.preset_labels ?? []).concat(presets.saved)
            textRole: "label"
            valueRole: "value"
            displayText: presets.daemon.tr("Presets")
            onActivated: presets.daemon.act("ApplyScenePreset", [presets.output, currentValue])

            delegate: QQC2.ItemDelegate {
                id: preset

                required property var modelData
                required property int index

                width: ListView.view?.width ?? implicitWidth
                text: modelData.label
                highlighted: chooser.highlightedIndex === index

                contentItem: QQC2.Label {
                    text: preset.text
                    color: preset.highlighted ? Kirigami.Theme.highlightedTextColor : Kirigami.Theme.textColor
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                }
            }
        }

        QQC2.Button {
            icon.name: "document-save"
            text: presets.daemon.tr("Save as preset…")
            onClicked: naming.open()
        }

        QQC2.ToolButton {
            icon.name: "edit-copy"
            text: presets.daemon.tr("Copy settings")
            onClicked: presets.exported(text => presets.copy(text))
        }

        QQC2.ToolButton {
            icon.name: "edit-paste"
            text: presets.daemon.tr("Paste settings")
            onClicked: presets.imported(presets.paste())
        }

        QQC2.ToolButton {
            icon.name: "document-export"
            text: presets.daemon.tr("Export…")
            display: QQC2.AbstractButton.IconOnly
            onClicked: writing.open()
        }

        QQC2.ToolButton {
            icon.name: "document-import"
            text: presets.daemon.tr("Import…")
            display: QQC2.AbstractButton.IconOnly
            onClicked: reading.open()
        }

        QQC2.Button {
            icon.name: "edit-undo"
            text: presets.daemon.tr("Reset")
            onClicked: presets.daemon.act("ResetSceneProperties", [presets.output])
        }
    }

    QQC2.CheckBox {
        visible: presets.output !== ""
        text: presets.daemon.tr("Same settings on every screen")
        checked: presets.props.shared === true
        onToggled: presets.daemon.act("ShareSceneValues", [presets.output, checked])
    }

    SavedPresets {
        Layout.fillWidth: true
        daemon: presets.daemon
        output: presets.output
        list: presets.saved
        onNotice: text => presets.notice = text
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: presets.notice !== ""
        text: presets.notice
        showCloseButton: true
        onVisibleChanged: if (!visible)
            presets.notice = ""
    }

    Kirigami.PromptDialog {
        id: naming

        title: presets.daemon.tr("Save the current settings as a preset")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: presets.daemon.act("SaveScenePreset", [presets.output, name.text])

        QQC2.TextField {
            id: name

            placeholderText: presets.daemon.tr("Name")
        }
    }

    FileDialog {
        id: writing

        fileMode: FileDialog.SaveFile
        defaultSuffix: "json"
        nameFilters: [presets.daemon.tr("Settings") + " (*.json)"]
        onAccepted: presets.exported(text => presets.notice = presets.daemon.writeText(selectedFile, text) ? "" : presets.daemon.tr("Cannot write the file"))
    }

    FileDialog {
        id: reading

        nameFilters: [presets.daemon.tr("Settings") + " (*.json)"]
        onAccepted: presets.imported(presets.daemon.readText(selectedFile))
    }
}
