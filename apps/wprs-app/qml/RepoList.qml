import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

QQC2.ScrollView {
    id: list

    required property var repos
    required property string error

    signal remove(string repo)

    ListView {
        model: list.repos
        spacing: Kirigami.Units.largeSpacing

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: parent.count === 0
            icon.name: list.error ? "data-error" : "package-x-generic"
            text: list.error || App.tr("No repositories yet")
            explanation: list.error ? "" : App.tr("Add a repository with the link its publisher gives you, including its fingerprint.")
        }

        delegate: ColumnLayout {
            required property var modelData

            width: ListView.view.width

            Kirigami.Heading {
                level: 4
                text: modelData.name + " · " + App.tr("Packages: %1").arg(modelData.packages)
                textFormat: Text.PlainText
            }

            QQC2.Label {
                Layout.fillWidth: true
                wrapMode: Text.WrapAnywhere
                text: [App.tr("Contacts: %1").arg(modelData.hosts.join(", ")), App.tr("Keys: %1").arg(modelData.fingerprints.join(", ")), App.tr("Last check: %1").arg(App.time(modelData.checked))].join("\n")
                textFormat: Text.PlainText
            }

            QQC2.Label {
                Layout.fillWidth: true
                visible: text !== ""
                wrapMode: Text.Wrap
                color: Kirigami.Theme.negativeTextColor
                text: modelData.warnings.map(w => Labels.value(w)).concat(modelData.last_error ? [modelData.last_error] : []).join("\n")
                textFormat: Text.PlainText
            }

            RowLayout {
                QQC2.Button {
                    text: App.tr("Refresh")
                    icon.name: "view-refresh"
                    onClicked: App.act("Refresh", [modelData.repo_id], "Repos1")
                }

                QQC2.Button {
                    text: App.tr("Remove…")
                    icon.name: "edit-delete"
                    onClicked: list.remove(modelData.repo_id)
                }
            }
        }
    }
}
