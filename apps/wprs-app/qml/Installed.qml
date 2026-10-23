import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: installed

    required property Installer installer
    property var packages: []
    readonly property var updates: packages.filter((p, i) => p.update && packages.findIndex(q => q.id === p.id) === i)

    function load() {
        App.query("ListInstalled", [], "Repos1", reply => installed.packages = reply.packages);
    }

    Component.onCompleted: load()

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "repos")
                installed.load();
        }
    }

    QQC2.Button {
        text: App.tr("Update all")
        icon.name: "system-software-update"
        enabled: installed.updates.length > 0
        onClicked: installed.updates.forEach(p => installed.installer.install(p.repo_id + "/" + p.id))
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            model: installed.packages

            delegate: RowLayout {
                id: row

                required property var modelData

                width: ListView.view.width

                ColumnLayout {
                    Layout.fillWidth: true

                    QQC2.Label {
                        Layout.fillWidth: true
                        text: row.modelData.id + " " + row.modelData.version + (row.modelData.revoked ? " · " + App.tr("revoked: %1").arg(Labels.value(row.modelData.revoked)) : "")
                        textFormat: Text.PlainText
                        color: row.modelData.revoked ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                        elide: Text.ElideRight
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        visible: !!row.modelData.update && text !== ""
                        text: row.modelData.changes ? App.tr("What's new: %1").arg(App.plain(row.modelData.changes)) : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font: Kirigami.Theme.smallFont
                    }
                }

                QQC2.Button {
                    visible: !!row.modelData.update
                    text: App.tr("Update to %1").arg(row.modelData.update ?? "")
                    onClicked: installed.installer.install(row.modelData.repo_id + "/" + row.modelData.id)
                }

                QQC2.ToolButton {
                    icon.name: "edit-delete"
                    text: App.tr("Uninstall")
                    onClicked: App.act("Uninstall", [row.modelData.id, row.modelData.version], "Repos1")
                }

                QQC2.ToolButton {
                    icon.name: "edit-delete-remove"
                    text: App.tr("Uninstall all versions")
                    display: QQC2.AbstractButton.IconOnly
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    onClicked: App.act("Uninstall", [row.modelData.id, ""], "Repos1")
                }
            }
        }
    }
}
