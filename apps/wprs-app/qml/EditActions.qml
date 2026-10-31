import QtQuick
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: bar

    required property var page
    readonly property var editor: page.editor
    readonly property list<Kirigami.Action> actions: [
        Kirigami.Action {
            text: App.tr("Back")
            icon.name: "go-previous"
            onTriggered: App.navigate(0, "")
        },
        Kirigami.Action {
            text: App.tr("Save")
            icon.name: "document-save"
            enabled: bar.editor.dirty
            onTriggered: bar.editor.save()
        },
        Kirigami.Action {
            text: App.tr("Undo")
            icon.name: "edit-undo"
            shortcut: StandardKey.Undo
            enabled: bar.editor.canUndo
            onTriggered: bar.editor.undo()
        },
        Kirigami.Action {
            text: App.tr("Redo")
            icon.name: "edit-redo"
            shortcut: StandardKey.Redo
            enabled: bar.editor.canRedo
            onTriggered: bar.editor.redo()
        },
        Kirigami.Action {
            text: App.tr("Compare with the original")
            icon.name: "view-split-left-right"
            shortcut: "\\"
            checkable: true
            checked: bar.editor.comparing
            enabled: bar.editor.ready
            onTriggered: bar.editor.compare(checked)
        },
        Kirigami.Action {
            text: App.tr("Show on the desktop")
            icon.name: "view-visible"
            checkable: true
            checked: bar.editor.live
            enabled: bar.editor.ready
            onTriggered: bar.editor.preview(!checked)
        },
        Kirigami.Action {
            text: App.tr("Export…")
            icon.name: "document-export"
            enabled: bar.editor.ready
            onTriggered: files.open()
        },
        Kirigami.Action {
            text: App.tr("Reset all")
            icon.name: "edit-reset"
            enabled: bar.editor.ready
            onTriggered: bar.page.run("reset")
        }
    ]

    FileDialog {
        id: files

        fileMode: FileDialog.SaveFile
        defaultSuffix: "png"
        nameFilters: [App.tr("PNG pictures") + " (*.png)", App.tr("JPEG pictures") + " (*.jpg *.jpeg)", App.tr("Lossless WebP pictures") + " (*.webp)", App.tr("AVIF pictures") + " (*.avif)"]
        onAccepted: bar.editor.saveAs(selectedFile.toString(), 90)
    }
}
