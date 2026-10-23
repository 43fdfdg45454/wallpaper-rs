import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.ScrollablePage {
    id: page

    property var sources: []
    property var inputs: ({})
    readonly property var grants: inputs.grants ?? {}

    function load() {
        App.query("ListSources", [], "Library1", reply => page.sources = reply.sources);
        App.query("GetInputs", [], "Inputs1", reply => page.inputs = reply);
    }

    title: App.tr("Settings")
    Component.onCompleted: load()
    actions: [
        Kirigami.Action {
            text: App.tr("Add folder…")
            icon.name: "folder-new"
            onTriggered: folders.open()
        },
        Kirigami.Action {
            text: App.tr("Reload configuration")
            icon.name: "view-refresh"
            onTriggered: App.act("Reload", [])
        }
    ]

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "library" || topic === "inputs")
                page.load();
        }
    }

    ColumnLayout {
        Screens {
            Layout.fillWidth: true
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("General")
            }

            Settings {
                daemon: App
                model: App.schema.options ?? []
                values: App.state.options ?? {}
                onEdited: (key, value) => App.act("SetOption", [key, value])
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("Library folders")
            }

            Repeater {
                model: page.sources

                delegate: RowLayout {
                    required property var modelData

                    QQC2.Label {
                        text: [modelData.path, modelData.recursive ? "" : App.tr("without subfolders"), modelData.provider ?? "", App.tr("Items: %1").arg(modelData.items)].filter(t => t).join(" · ")
                        color: modelData.error ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                    }

                    QQC2.ToolButton {
                        icon.name: "view-refresh"
                        text: App.tr("Rescan")
                        display: QQC2.AbstractButton.IconOnly
                        onClicked: App.act("Rescan", [modelData.uid], "Library1")
                    }

                    QQC2.ToolButton {
                        visible: !modelData.provider
                        icon.name: "list-remove"
                        text: App.tr("Remove")
                        display: QQC2.AbstractButton.IconOnly
                        onClicked: App.act("RemoveSource", [modelData.uid], "Library1")
                    }
                }
            }

            QQC2.Switch {
                id: recursive

                text: App.tr("Include subfolders")
                checked: true
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("Scene permissions")
            }

            QQC2.Label {
                Layout.maximumWidth: Kirigami.Units.gridUnit * 30
                wrapMode: Text.Wrap
                text: [App.tr("A scene follows the sound that plays or the pointer only after you allow it for that scene, and only while it is shown. The sound is turned into levels on this computer and never recorded, kept or sent; pointer positions are never kept."), page.inputs.capturing ? App.tr("A scene is following the sound now.") : "", page.inputs.tracking ? App.tr("A scene is following the pointer now.") : "", Object.keys(page.grants).length ? "" : App.tr("No scene has been allowed yet.")].filter(t => t).join("\n")
            }

            Repeater {
                model: Object.keys(page.grants)

                delegate: RowLayout {
                    id: grant

                    required property string modelData

                    QQC2.Label {
                        text: grant.modelData
                    }

                    Repeater {
                        model: page.grants[grant.modelData]

                        delegate: QQC2.Button {
                            required property string modelData

                            icon.name: "dialog-cancel"
                            text: App.tr("Stop using %1").arg(Labels.value(modelData))
                            onClicked: App.act("Revoke", [grant.modelData, modelData], "Inputs1")
                        }
                    }
                }
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("About")
            }

            QQC2.Label {
                Kirigami.FormData.label: App.tr("Version:")
                text: [App.state.version, App.state.adapter, App.tr("data model %1").arg(App.state.model ?? "")].filter(t => t).join(" · ")
            }

            QQC2.Label {
                Kirigami.FormData.label: App.tr("Configuration:")
                visible: !!App.state.config_error || !!App.state.library_error
                text: [App.state.config_error, App.state.library_error].filter(t => t).join("\n")
                color: Kirigami.Theme.negativeTextColor
                wrapMode: Text.Wrap
            }
        }
    }

    FolderDialog {
        id: folders

        onAccepted: App.act("AddSource", [App.path(selectedFolder), recursive.checked], "Library1")
    }
}
