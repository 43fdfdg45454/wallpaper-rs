import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: entry

    required property var daemon
    required property string output
    required property var modelData
    readonly property string kind: modelData.kind
    readonly property string text: String(modelData.value ?? "")
    readonly property bool secret: modelData.secret === true
    readonly property var accept: modelData.accept ?? []
    readonly property string images: "*.png *.jpg *.jpeg *.webp *.gif *.bmp"
    readonly property string videos: "*.mp4 *.m4v *.webm *.mkv *.mov *.avi *.ogv *.mpg *.mpeg"
    property string notice: ""

    signal edited(string key, var value)

    function keep(text) {
        daemon.sceneSecret(output, modelData.key, text, () => entry.notice = "", error => entry.notice = error.message);
    }

    QQC2.TextField {
        Layout.fillWidth: true
        visible: entry.kind === "text" && !entry.modelData.multiline && !entry.secret
        text: entry.text
        maximumLength: entry.modelData.max_len ?? 1024
        onEditingFinished: {
            if (text !== entry.text)
                entry.edited(entry.modelData.key, text);
        }
    }

    Area {
        Layout.fillWidth: true
        visible: entry.kind === "text" && entry.modelData.multiline && !entry.secret
        text: entry.text
        onEditingFinished: {
            if (text !== entry.text)
                entry.edited(entry.modelData.key, text);
        }
    }

    RowLayout {
        Layout.fillWidth: true
        visible: entry.secret

        Kirigami.PasswordField {
            id: hidden

            Layout.fillWidth: true
            placeholderText: entry.modelData.filled ? entry.daemon.tr("Stored in the keyring") : ""
            onAccepted: {
                entry.keep(text);
                text = "";
            }
        }

        QQC2.Button {
            text: entry.daemon.tr("Save")
            enabled: hidden.text !== ""
            onClicked: hidden.accepted()
        }

        QQC2.ToolButton {
            visible: entry.modelData.filled === true
            icon.name: "edit-clear"
            text: entry.daemon.tr("Forget")
            display: QQC2.AbstractButton.IconOnly
            onClicked: entry.keep("")
        }
    }

    RowLayout {
        visible: entry.kind === "file" || entry.kind === "folder"

        QQC2.Button {
            text: entry.text.split("/").pop() || (entry.kind === "folder" ? entry.daemon.tr("Choose a folder…") : entry.daemon.tr("Choose…"))
            icon.name: entry.kind === "folder" ? "folder-open" : "document-open"
            onClicked: entry.kind === "folder" ? folders.open() : files.open()
        }

        QQC2.ToolButton {
            visible: entry.text !== ""
            icon.name: "edit-clear"
            text: entry.daemon.tr("Use the original")
            display: QQC2.AbstractButton.IconOnly
            onClicked: entry.edited(entry.modelData.key, null)
        }
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: entry.notice !== ""
        type: Kirigami.MessageType.Error
        text: entry.notice
    }

    FileDialog {
        id: files

        nameFilters: {
            const images = entry.accept.includes("image");
            const videos = entry.accept.includes("video");
            const both = images && videos ? entry.daemon.tr("Images and videos") + " (" + entry.images + " " + entry.videos + ")" : "";
            return [both, images ? entry.daemon.tr("Images") + " (" + entry.images + ")" : "", videos ? entry.daemon.tr("Videos") + " (" + entry.videos + ")" : ""].filter(f => f !== "");
        }
        onAccepted: entry.edited(entry.modelData.key, entry.daemon.path(selectedFile))
    }

    FolderDialog {
        id: folders

        onAccepted: entry.edited(entry.modelData.key, entry.daemon.path(selectedFolder))
    }
}
