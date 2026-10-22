import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.ScrollablePage {
    id: page

    property var providers: []
    property string error

    function load() {
        App.query("List", [], "Providers1", reply => {
            page.providers = reply.providers;
            page.error = "";
        }, error => page.error = error.message);
    }

    function exercise() {
        adder.open();
    }

    title: App.tr("Providers")
    Component.onCompleted: load()
    actions: [
        Kirigami.Action {
            text: App.tr("Add provider…")
            icon.name: "list-add"
            onTriggered: adder.open()
        },
        Kirigami.Action {
            text: App.tr("Sync all")
            icon.name: "view-refresh"
            enabled: page.providers.length > 0
            onTriggered: App.act("Sync", [""], "Providers1")
        }
    ]

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "providers")
                page.load();
        }
    }

    ColumnLayout {
        Kirigami.PlaceholderMessage {
            Layout.fillWidth: true
            visible: page.providers.length === 0
            icon.name: page.error ? "data-error" : "internet-services"
            text: page.error || App.tr("No providers yet")
            explanation: page.error ? "" : App.tr("Providers download pictures from online sources you choose into your library.")
        }

        Repeater {
            model: page.providers

            delegate: ProviderCard {
                required property var modelData

                Layout.fillWidth: true
                provider: modelData
            }
        }
    }

    ProviderDialog {
        id: adder
    }
}
