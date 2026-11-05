import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            model: panel.page.view.assets ?? []
            clip: true

            delegate: QQC2.ItemDelegate {
                id: row

                required property var modelData

                width: ListView.view.width
                text: modelData.id + " · " + SceneWords.kind(modelData.kind) + " · " + App.bytes(modelData.size) + (modelData.media ? " · " + (modelData.media === "current" ? App.tr("cover of what plays") : App.tr("previous cover")) : "")
                highlighted: panel.page.editor.selected === modelData.pointer
                onClicked: panel.page.editor.select(modelData.pointer)

                QQC2.ToolButton {
                    anchors.right: remove.left
                    visible: row.modelData.kind === "image" && !row.modelData.path.toLowerCase().endsWith(".ktx2")
                    icon.name: "application-x-archive"
                    text: App.tr("Convert to KTX2…")
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    onClicked: packing.show(row.modelData.id, row.modelData.pointer)
                }

                QQC2.ToolButton {
                    id: remove

                    anchors.right: parent.right
                    icon.name: "edit-delete"
                    text: App.tr("Remove")
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: panel.page.run("remove", {
                        "pointer": row.modelData.pointer
                    })
                }
            }
        }
    }

    Flow {
        Layout.fillWidth: true

        QQC2.ToolButton {
            text: App.tr("Import…")
            icon.name: "document-import"
            onClicked: files.open()
        }

        QQC2.ToolButton {
            text: App.tr("Import an effect…")
            icon.name: "special-effects-symbolic"
            onClicked: shaders.open()
        }

        QQC2.ToolButton {
            text: App.tr("Import a script…")
            icon.name: "text-x-script"
            onClicked: modules.open()
        }

        QQC2.ToolButton {
            text: App.tr("Import from an assets package…")
            icon.name: "package-x-generic"
            onClicked: shared.open()
        }

        QQC2.ToolButton {
            text: App.tr("Make an assets package…")
            icon.name: "document-export"
            onClicked: offer.open()
        }

        QQC2.ToolButton {
            text: App.tr("Remove unused files")
            icon.name: "edit-clear-all"
            onClicked: panel.page.editor.prune()
        }
    }

    SceneShared {
        id: shared

        page: panel.page
    }

    SceneOffer {
        id: offer

        page: panel.page
    }

    SceneKtx2 {
        id: packing

        page: panel.page
    }

    FileDialog {
        id: files

        fileMode: FileDialog.OpenFiles
        nameFilters: [App.tr("Pictures, videos, sounds, fonts, color tables and models") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.ktx2 *.webm *.mp4 *.mkv *.ogg *.opus *.flac *.wav *.ttf *.otf *.cube *.glb)"]
        onAccepted: selectedFiles.forEach(file => panel.page.run("import", {
                "file": App.path(file)
            }))
    }

    FileDialog {
        id: modules

        fileMode: FileDialog.OpenFile
        nameFilters: [App.tr("A JavaScript module of the scene") + " (*.js)"]
        onAccepted: panel.page.run("import_script", {
            "file": App.path(selectedFile)
        })
    }

    FileDialog {
        id: shaders

        fileMode: FileDialog.OpenFiles
        nameFilters: [App.tr("An effect descriptor, and its WGSL shader when it is not beside it") + " (*.json *.wgsl)"]
        onAccepted: {
            const paths = selectedFiles.map(file => App.path(file));
            const descriptor = paths.find(path => path.endsWith(".json"));
            const wgsl = paths.find(path => path.endsWith(".wgsl"));
            if (descriptor)
                panel.page.run("import_effect", wgsl ? {
                    "descriptor": descriptor,
                    "wgsl": wgsl
                } : {
                    "descriptor": descriptor
                });
        }
    }
}
