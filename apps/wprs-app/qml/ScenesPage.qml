import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.ScrollablePage {
    id: page

    property var scenes: []
    property var projects: JSON.parse(helper.projects() || "[]")
    readonly property var starters: [["empty", "document-new"], ["particles", "weather-snow"], ["parallax", "view-list-details"], ["clock", "clock"]]

    function exercise() {
        App.compose(helper.folderFor("check"), "empty", "");
    }

    title: App.tr("Scenes")
    Component.onCompleted: App.query("Search", ["kind:scene", 0, 200], "Library1", reply => {
            if (page)
                page.scenes = (reply?.items ?? []).filter(item => String(item.path ?? "").endsWith(".wpak"));
        })

    SceneEditor {
        id: helper
    }

    FolderDialog {
        id: folders

        onAccepted: App.compose(App.path(selectedFolder), "", "")
    }

    ColumnLayout {
        Kirigami.FormLayout {
            Layout.fillWidth: true

            QQC2.TextField {
                id: name

                Kirigami.FormData.label: App.tr("Title:")
                text: App.tr("My scene")
            }

            QQC2.TextField {
                id: folder

                Kirigami.FormData.label: App.tr("Folder:")
                Layout.fillWidth: true
                text: helper.folderFor(name.text)
            }
        }

        Flow {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            Repeater {
                model: page.starters

                delegate: QQC2.Button {
                    required property var modelData

                    text: SceneWords.kind(modelData[0])
                    icon.name: modelData[1]
                    display: QQC2.AbstractButton.TextUnderIcon
                    implicitWidth: Kirigami.Units.gridUnit * 9
                    implicitHeight: Kirigami.Units.gridUnit * 5
                    onClicked: App.compose(folder.text, modelData[0], name.text)
                }
            }
        }

        Kirigami.Heading {
            level: 3
            text: App.tr("Open")
        }

        QQC2.Button {
            text: App.tr("Open a scene folder…")
            icon.name: "document-open-folder"
            onClicked: folders.open()
        }

        Repeater {
            model: page.projects

            delegate: QQC2.ItemDelegate {
                required property var modelData

                Layout.fillWidth: true
                text: (modelData.title || "") + " — " + modelData.folder
                icon.name: "folder"
                onClicked: App.compose(modelData.folder, "", "")
            }
        }

        Kirigami.Heading {
            level: 3
            text: App.tr("Edit a copy of a scene from the library")
            visible: page.scenes.length > 0
        }

        Repeater {
            model: page.scenes

            delegate: QQC2.ItemDelegate {
                required property var modelData

                Layout.fillWidth: true
                text: modelData.title
                icon.name: "edit-copy"
                onClicked: App.compose(helper.folderFor(modelData.title), "", modelData.path)
            }
        }
    }
}
