import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    readonly property var kinds: App.schema.providers ?? []
    readonly property var kind: kinds[chooser.currentIndex] ?? {}
    readonly property var fields: (kind.options ?? []).filter(field => field.key !== "allow_plain_http")
    readonly property bool plain: (kind.options ?? []).some(field => field.key === "allow_plain_http")
    property var values: ({})
    property string error

    title: App.tr("Add provider")
    standardButtons: Kirigami.Dialog.Cancel
    customFooterActions: [
        Kirigami.Action {
            text: App.tr("Add")
            icon.name: "list-add"
            onTriggered: {
                const options = Object.assign({}, dialog.values);
                if (dialog.plain)
                    options.allow_plain_http = String(insecure.checked);
                App.addProvider(name.text, dialog.kind.kind, options, secret.text, () => dialog.close(), error => dialog.error = error.message);
                secret.clear();
            }
        }
    ]
    onOpened: {
        values = {};
        error = "";
    }
    onClosed: secret.clear()

    Kirigami.FormLayout {
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: Kirigami.MessageType.Error
            visible: dialog.error !== ""
            text: dialog.error
        }

        QQC2.TextField {
            id: name

            Kirigami.FormData.label: App.tr("Name:")
            placeholderText: App.tr("lowercase letters, digits, - and _")
            validator: RegularExpressionValidator {
                regularExpression: /[a-z][a-z0-9_-]{0,31}/
            }
        }

        QQC2.ComboBox {
            id: chooser

            Kirigami.FormData.label: App.tr("Kind:")
            model: dialog.kinds.map(kind => kind.kind)
            onActivated: dialog.values = {}
        }

        Settings {
            daemon: App
            model: dialog.fields
            values: dialog.values
            onEdited: (key, value) => dialog.values[key] = value
        }

        Kirigami.PasswordField {
            id: secret

            Kirigami.FormData.label: Labels.field(dialog.kind.secret?.name ?? "password") + ":"
            visible: !!dialog.kind.secret
            placeholderText: dialog.kind.secret?.required ? App.tr("Required") : App.tr("Optional")
        }

        QQC2.CheckBox {
            id: insecure

            Kirigami.FormData.label: App.tr("Advanced:")
            visible: dialog.plain
            text: Labels.field("allow_plain_http")
        }

        QQC2.Label {
            visible: insecure.visible && insecure.checked
            Layout.maximumWidth: Kirigami.Units.gridUnit * 18
            text: App.tr("Without encryption the secret and the pictures travel in clear text over the network.")
            color: Kirigami.Theme.negativeTextColor
            wrapMode: Text.Wrap
        }
    }
}
