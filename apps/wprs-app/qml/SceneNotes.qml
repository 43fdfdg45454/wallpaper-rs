import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: notes

    required property var page

    spacing: 0

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        type: Kirigami.MessageType.Error
        text: notes.page.editor.error
        visible: text !== ""
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        type: Kirigami.MessageType.Positive
        text: notes.page.notice
        visible: text !== ""
        showCloseButton: true
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        type: Kirigami.MessageType.Warning
        text: App.tr("This folder has unsaved changes from an earlier session.")
        visible: notes.page.view.recoverable === true
        actions: [
            Kirigami.Action {
                text: App.tr("Recover")
                onTriggered: notes.page.editor.recover(true)
            },
            Kirigami.Action {
                text: App.tr("Discard")
                onTriggered: notes.page.editor.recover(false)
            }
        ]
    }
}
