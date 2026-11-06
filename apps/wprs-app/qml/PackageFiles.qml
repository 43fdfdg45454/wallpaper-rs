import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    readonly property var files: page.view.files ?? []
    readonly property var icons: ({
            "page": "text-html",
            "script": "text-x-javascript",
            "style": "text-css",
            "image": "image-x-generic",
            "video": "video-x-generic",
            "sound": "audio-x-generic",
            "font": "font-x-generic",
            "data": "application-json"
        })

    function roleText(role) {
        return ({
                "entry": App.tr("entry page"),
                "preload": App.tr("runs before the page"),
                "preview": App.tr("preview"),
                "manifest": App.tr("manifest"),
                "locale": App.tr("texts of a language"),
                "signature": App.tr("signature")
            })[role] ?? "";
    }

    function size(bytes) {
        return bytes < 1024 ? App.tr("%1 B", bytes) : bytes < 1048576 ? App.tr("%1 KiB", (bytes / 1024).toFixed(1)) : App.tr("%1 MiB", (bytes / 1048576).toFixed(1));
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: App.tr("%1 files, %2 in all", panel.files.length, panel.size(panel.files.reduce((sum, f) => sum + f.size, 0)))
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.Button {
            text: App.tr("Add files…")
            icon.name: "list-add"
            onClicked: picker.open()
        }

        QQC2.Button {
            text: App.tr("Open the folder")
            icon.name: "document-open-folder"
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: Qt.openUrlExternally("file://" + panel.page.editor.folder)
        }
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            id: list

            clip: true
            model: panel.files

            delegate: QQC2.ItemDelegate {
                id: row

                required property var modelData

                width: ListView.view.width
                icon.name: panel.icons[modelData.kind] ?? "text-x-generic"
                text: modelData.path
                onClicked: menu.popup()

                contentItem: ColumnLayout {
                    spacing: 0

                    QQC2.Label {
                        Layout.fillWidth: true
                        text: row.modelData.path
                        elide: Text.ElideMiddle
                        font.bold: row.modelData.role === "entry"
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        text: [panel.size(row.modelData.size), panel.roleText(row.modelData.role)].filter(t => t).join(" · ")
                        color: Kirigami.Theme.disabledTextColor
                        font: Kirigami.Theme.smallFont
                    }
                }

                QQC2.Menu {
                    id: menu

                    QQC2.MenuItem {
                        text: App.tr("Use as the entry page")
                        visible: row.modelData.kind === "page" && row.modelData.role !== "entry"
                        height: visible ? implicitHeight : 0
                        onTriggered: panel.page.change("/manifest/entry", row.modelData.path)
                    }

                    QQC2.MenuItem {
                        text: App.tr("Run before the page")
                        visible: panel.page.kind === "web" && row.modelData.kind === "script" && row.modelData.role !== "preload"
                        height: visible ? implicitHeight : 0
                        onTriggered: panel.page.manifestEdit("web", {
                            "values": {
                                "preload": row.modelData.path
                            }
                        })
                    }

                    QQC2.MenuItem {
                        text: App.tr("Use as the thumbnail")
                        visible: row.modelData.kind === "image"
                        height: visible ? implicitHeight : 0
                        onTriggered: panel.page.run("thumbnail", {
                            "file": panel.page.editor.folder + "/" + row.modelData.path
                        })
                    }

                    QQC2.MenuItem {
                        text: App.tr("Open")
                        onTriggered: Qt.openUrlExternally("file://" + panel.page.editor.folder + "/" + row.modelData.path)
                    }
                }
            }
        }
    }

    FileDialog {
        id: picker

        fileMode: FileDialog.OpenFiles
        onAccepted: {
            for (const file of selectedFiles)
                panel.page.manifestEdit("add_file", {
                    "file": App.path(file)
                });
        }
    }
}
