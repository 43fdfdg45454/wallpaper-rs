import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: saved

    required property var daemon
    required property string output
    required property var list
    property string chosen: ""
    property string text: ""

    signal notice(string text)

    Repeater {
        model: saved.list

        delegate: RowLayout {
            id: mine

            required property var modelData

            QQC2.Label {
                Layout.fillWidth: true
                text: mine.modelData.label
                textFormat: Text.PlainText
                elide: Text.ElideRight
            }

            QQC2.ToolButton {
                icon.name: "edit-rename"
                text: saved.daemon.tr("Rename")
                display: QQC2.AbstractButton.IconOnly
                onClicked: {
                    saved.chosen = mine.modelData.value;
                    renamed.text = mine.modelData.label;
                    renaming.open();
                }
            }

            QQC2.ToolButton {
                icon.name: "document-export"
                text: saved.daemon.tr("Export…")
                display: QQC2.AbstractButton.IconOnly
                onClicked: {
                    saved.chosen = mine.modelData.value;
                    writing.open();
                }
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                text: saved.daemon.tr("Delete")
                display: QQC2.AbstractButton.IconOnly
                onClicked: saved.daemon.act("DeleteScenePreset", [mine.modelData.value])
            }
        }
    }

    QQC2.Button {
        icon.name: "document-import"
        text: saved.daemon.tr("Import as preset…")
        onClicked: reading.open()
    }

    Kirigami.PromptDialog {
        id: renaming

        title: saved.daemon.tr("Rename the preset")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: saved.daemon.act("RenameScenePreset", [saved.chosen, renamed.text])

        QQC2.TextField {
            id: renamed
        }
    }

    Kirigami.PromptDialog {
        id: naming

        title: saved.daemon.tr("Name the imported preset")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: saved.daemon.query("ImportSceneValues", [saved.output, saved.text, name.text], "Daemon1", reply => saved && saved.notice((reply[0] ?? []).join("\n")), error => saved && saved.notice(error.message))

        QQC2.TextField {
            id: name

            placeholderText: saved.daemon.tr("Name")
        }
    }

    FileDialog {
        id: writing

        fileMode: FileDialog.SaveFile
        defaultSuffix: "json"
        nameFilters: [saved.daemon.tr("Settings") + " (*.json)"]
        onAccepted: saved.daemon.query("ExportSceneValues", [saved.output, saved.chosen], "Daemon1", reply => saved && saved.notice(saved.daemon.writeText(selectedFile, reply[0]) ? "" : saved.daemon.tr("Cannot write the file")), error => saved && saved.notice(error.message))
    }

    FileDialog {
        id: reading

        nameFilters: [saved.daemon.tr("Settings") + " (*.json)"]
        onAccepted: {
            saved.text = saved.daemon.readText(selectedFile);
            name.text = "";
            naming.open();
        }
    }
}
