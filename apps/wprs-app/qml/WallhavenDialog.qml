import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    required property var provider
    required property var kind
    readonly property var fields: (kind.options ?? []).filter(field => field.key !== "collection")
    readonly property var shown: Object.assign({}, provider.options, values)
    property var values: ({})
    property var found: []
    property string error

    function list(owner) {
        App.query("Collections", [provider.id, owner], "Providers1", reply => {
                if (!dialog)
                    return;
                dialog.found = reply.collections;
                dialog.error = "";
            }, e => dialog && (dialog.error = e.message));
    }

    function choose(collection) {
        values = Object.assign({}, values, {
                "collection": collection
            });
    }

    title: App.tr("Wallhaven search of %1", provider.id)
    preferredWidth: Kirigami.Units.gridUnit * 38
    standardButtons: Kirigami.Dialog.Cancel
    customFooterActions: [
        Kirigami.Action {
            text: App.tr("Apply")
            icon.name: "dialog-ok-apply"
            onTriggered: App.act("Update", [dialog.provider.id, dialog.values], "Providers1", () => dialog && dialog.close(), e => dialog && (dialog.error = e.message))
        },
        Kirigami.Action {
            text: App.tr("New random order")
            icon.name: "roll"
            onTriggered: App.act("Reseed", [dialog.provider.id], "Providers1")
        }
    ]
    onOpened: {
        values = {};
        found = [];
        error = "";
        owner.text = (provider.options.collection ?? "").split("/")[0];
    }

    Kirigami.FormLayout {
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            type: Kirigami.MessageType.Error
            visible: dialog.error !== ""
            text: dialog.error
        }

        Settings {
            daemon: App
            model: dialog.fields
            values: dialog.shown
            onEdited: (key, value) => dialog.values = Object.assign({}, dialog.values, {
                        [key]: value
                    })
        }

        QQC2.Label {
            Kirigami.FormData.label: Labels.field("collection") + ":"
            text: dialog.shown.collection || App.tr("none")
            textFormat: Text.PlainText
        }

        RowLayout {
            Kirigami.FormData.label: App.tr("Collections of:")

            QQC2.TextField {
                id: owner

                placeholderText: App.tr("Wallhaven user name")
                validator: RegularExpressionValidator {
                    regularExpression: /[A-Za-z0-9_-]{0,32}/
                }
            }

            QQC2.Button {
                text: App.tr("List")
                icon.name: "view-list-details"
                enabled: owner.text !== "" || dialog.provider.has_secret
                onClicked: dialog.list(owner.text)
            }

            QQC2.Button {
                text: App.tr("Search instead")
                icon.name: "edit-clear"
                visible: !!dialog.shown.collection
                onClicked: dialog.choose("")
            }
        }

        Repeater {
            model: dialog.found

            delegate: RowLayout {
                required property var modelData

                QQC2.RadioButton {
                    enabled: owner.text !== ""
                    checked: dialog.shown.collection === owner.text + "/" + modelData.id
                    onClicked: dialog.choose(owner.text + "/" + modelData.id)
                }

                QQC2.Label {
                    text: App.tr("%1 (%2 pictures)", modelData.label, modelData.count) + (modelData["public"] ? "" : " · " + App.tr("private"))
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 24
                }
            }
        }
    }
}
