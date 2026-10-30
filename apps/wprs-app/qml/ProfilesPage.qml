import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.ScrollablePage {
    id: page

    property var profiles: []
    property var presets: []
    property string active

    function load() {
        App.query("List", [], "Profiles1", reply => {
            if (!page)
                return;
            page.profiles = reply.profiles;
            page.active = reply.active ?? "";
        });
        App.query("ListPresets", [""], "Profiles1", reply => page && (page.presets = reply.presets));
    }

    title: App.tr("Profiles and presets")
    Component.onCompleted: load()
    actions: [
        Kirigami.Action {
            text: App.tr("Save current as…")
            icon.name: "document-save-as"
            onTriggered: saver.open()
        },
        Kirigami.Action {
            text: App.tr("No profile")
            icon.name: "edit-clear"
            enabled: page.active !== ""
            onTriggered: App.act("Activate", [""], "Profiles1")
        },
        Kirigami.Action {
            text: App.tr("New preset…")
            icon.name: "list-add"
            onTriggered: preset.open()
        }
    ]

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "profiles")
                page.load();
        }
    }

    ColumnLayout {
        Kirigami.Heading {
            text: App.tr("Profiles")
        }

        Repeater {
            model: page.profiles

            delegate: ProfileCard {
                required property var modelData

                Layout.fillWidth: true
                profile: modelData
            }
        }

        Kirigami.Heading {
            text: App.tr("Global presets")
        }

        Repeater {
            model: page.presets

            delegate: RowLayout {
                required property var modelData

                QQC2.Label {
                    Layout.fillWidth: true
                    text: [modelData.name, Labels.value(modelData.value.fit), modelData.value.background].filter(t => t).join(" · ")
                }

                QQC2.ToolButton {
                    icon.name: "edit-delete"
                    text: App.tr("Delete")
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: App.act("DeletePreset", [modelData.uid], "Profiles1")
                }
            }
        }
    }

    PresetDialog {
        id: preset
    }

    Kirigami.Dialog {
        id: saver

        title: App.tr("Save the current setup as a profile")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Save", [name.text, include.checked], "Profiles1")

        Kirigami.FormLayout {
            QQC2.TextField {
                id: name

                Kirigami.FormData.label: App.tr("Name:")
            }

            QQC2.CheckBox {
                id: include

                text: App.tr("Include settings")
                checked: true
            }
        }
    }
}
