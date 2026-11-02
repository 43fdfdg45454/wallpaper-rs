import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: uploads

    required property var account
    property var items: []

    function load() {
        App.query("ListPackages", [], "Sync1", reply => uploads && (uploads.items = reply.items ?? []), () => uploads && (uploads.items = []));
    }

    visible: uploads.items.length > 0
    Component.onCompleted: load()

    Connections {
        target: App

        function onUpdated(topic) {
            if (["account", "jobs"].includes(topic))
                uploads.load();
        }
    }

    Kirigami.Heading {
        level: 3
        text: App.tr("Your uploads")
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Unlisted versions stay out of the catalog and search; whoever has their link can still install them.")
    }

    Repeater {
        model: uploads.items

        delegate: RowLayout {
            required property var modelData

            Layout.fillWidth: true

            QQC2.Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: App.tr("%1 %2 · %3", modelData.title || modelData.package, modelData.version, modelData.state)
            }

            QQC2.ComboBox {
                model: [App.tr("Public"), App.tr("Unlisted")]
                currentIndex: modelData.visibility === "unlisted" ? 1 : 0
                onActivated: index => App.act("SetVisibility", [modelData.package, modelData.version, index === 1 ? "unlisted" : "public"], "Sync1", () => uploads && uploads.load())
            }
        }
    }
}
