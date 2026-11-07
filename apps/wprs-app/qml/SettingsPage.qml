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

    function load() {
        App.query("ListSources", [], "Library1", reply => page && (page.sources = reply.sources));
        App.query("GetInputs", [], "Inputs1", reply => page && (page.inputs = reply));
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
                model: (App.schema.options ?? []).filter(f => !["render.gpu", "surfaces.layer_namespace", "playback.quality", "playback.auto_pace", "playback.hdr", "rgb.enabled", "rgb.server", "rgb.when_covered", "media.remote_covers"].includes(f.key))
                values: App.state.options ?? {}
                onEdited: (key, value) => App.act("SetOption", [key, value])
            }

            FillNote {
                Layout.fillWidth: true
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("Scene quality")
            }

            QualityPicker {
                Kirigami.FormData.label: App.tr("Quality:")
                daemon: App
            }

            PacePicker {
                Kirigami.FormData.label: App.tr("Frame rate:")
                daemon: App
            }

            HdrPicker {
                Kirigami.FormData.label: App.tr("HDR:")
                daemon: App
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("Graphics card")
            }

            GpuPicker {
                Kirigami.FormData.label: App.tr("Draw with:")
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("RGB lighting")
            }

            SettingsRgb {}

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("Desktop surfaces")
            }

            LayerNamespaceField {
                Kirigami.FormData.label: App.tr("Surface name:")
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
                        text: [modelData.path, modelData.recursive ? "" : App.tr("without subfolders"), modelData.provider ?? "", App.tr("Items: %1", modelData.items)].filter(t => t).join(" · ")
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

            SettingsGrants {
                inputs: page.inputs
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("Media player")
            }

            SettingsMedia {
                inputs: page.inputs
            }

            Kirigami.Separator {
                Kirigami.FormData.isSection: true
                Kirigami.FormData.label: App.tr("About")
            }

            QQC2.Label {
                Kirigami.FormData.label: App.tr("Version:")
                text: [App.state.version, App.state.adapter, App.tr("data model %1", App.state.model ?? "")].filter(t => t).join(" · ")
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
