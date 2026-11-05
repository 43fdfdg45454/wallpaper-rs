import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: revoking

    property string subject: ""
    property bool noted: true
    readonly property var reasons: ["malware", "license", "takedown", "broken", "other"]

    signal revoke(string reason, string message)

    function ask(text, withMessage) {
        subject = text;
        noted = withMessage;
        message.text = "";
        open();
    }

    title: App.tr("Revoke")
    standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
    padding: Kirigami.Units.largeSpacing
    onAccepted: revoke(reasons[reason.currentIndex], noted ? message.text : "")

    Kirigami.FormLayout {
        QQC2.Label {
            Layout.maximumWidth: Kirigami.Units.gridUnit * 24
            wrapMode: Text.Wrap
            text: revoking.subject
        }

        QQC2.ComboBox {
            id: reason

            Kirigami.FormData.label: App.tr("Reason:")
            model: [App.tr("Malware"), App.tr("License"), App.tr("Takedown"), App.tr("Broken"), App.tr("Other")]
        }

        QQC2.TextField {
            id: message

            Kirigami.FormData.label: App.tr("Message for users:")
            visible: revoking.noted
            maximumLength: 500
            placeholderText: App.tr("Optional")
        }
    }
}
