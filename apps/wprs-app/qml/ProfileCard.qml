import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: card

    required property var profile
    property bool expanded

    RowLayout {
        Kirigami.Icon {
            source: card.profile.active ? "checkmark" : "view-list-details"
            implicitWidth: Kirigami.Units.iconSizes.small
            implicitHeight: implicitWidth
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: card.profile.name
            font.bold: card.profile.active
        }

        QQC2.Button {
            text: App.tr("Activate")
            enabled: !card.profile.active
            onClicked: App.act("Activate", [card.profile.uid], "Profiles1")
        }

        QQC2.ToolButton {
            icon.name: "chronometer"
            text: App.tr("Automatic rules")
            checkable: true
            checked: card.expanded
            onToggled: card.expanded = checked
        }

        QQC2.ToolButton {
            icon.name: "edit-rename"
            text: App.tr("Rename…")
            display: QQC2.AbstractButton.IconOnly
            onClicked: renamer.open()
        }

        QQC2.ToolButton {
            icon.name: "edit-delete"
            text: App.tr("Delete")
            display: QQC2.AbstractButton.IconOnly
            onClicked: App.act("Delete", [card.profile.uid], "Profiles1")
        }
    }

    Kirigami.FormLayout {
        Layout.fillWidth: true
        visible: card.expanded

        FieldForm {
            model: App.schema.auto ?? []
            values: Object.assign({
                "priority": card.profile.priority
            }, card.profile.auto)
            onEdited: (key, value) => App.act("SetAuto", [card.profile.uid, key, value], "Profiles1")
        }
    }

    Kirigami.Dialog {
        id: renamer

        title: App.tr("Rename profile")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Rename", [card.profile.uid, name.text], "Profiles1")

        QQC2.TextField {
            id: name

            text: card.profile.name
        }
    }
}
