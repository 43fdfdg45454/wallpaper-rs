import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: dialogs

    required property var provider
    required property var kind
    readonly property alias secret: secret
    readonly property alias option: option
    readonly property alias remover: remover

    Kirigami.Dialog {
        id: secret

        title: App.tr("Secret of %1").arg(dialogs.provider.id)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: {
            App.setSecret(dialogs.provider.id, password.text);
            password.clear();
        }
        onClosed: password.clear()

        Kirigami.PasswordField {
            id: password

            placeholderText: Labels.field(dialogs.kind.secret?.name ?? "password")
        }
    }

    Kirigami.Dialog {
        id: option

        readonly property var fields: dialogs.kind.options ?? []
        readonly property string key: fields[keys.currentIndex]?.key ?? ""

        title: App.tr("Change an option of %1").arg(dialogs.provider.id)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onOpened: value.text = dialogs.provider.options[key] ?? ""
        onAccepted: App.act("Set", [dialogs.provider.id, key, value.text], "Providers1")

        Kirigami.FormLayout {
            QQC2.ComboBox {
                id: keys

                Kirigami.FormData.label: App.tr("Option:")
                model: option.fields.map(field => Labels.field(field.key))
                onActivated: value.text = dialogs.provider.options[option.key] ?? ""
            }

            QQC2.TextField {
                id: value

                Kirigami.FormData.label: App.tr("Value:")
            }

            QQC2.Label {
                visible: !!dialogs.kind.secret && ["url", "server", "user"].includes(option.key)
                text: App.tr("Changing this forgets the stored secret.")
                color: Kirigami.Theme.neutralTextColor
            }
        }
    }

    Kirigami.Dialog {
        id: remover

        title: App.tr("Remove %1").arg(dialogs.provider.id)
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Remove", [dialogs.provider.id, purge.checked], "Providers1")

        QQC2.CheckBox {
            id: purge

            text: App.tr("Delete cached files")
        }
    }
}
