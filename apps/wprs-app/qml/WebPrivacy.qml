import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: privacy

    property string uid
    property var pkg: ({})
    readonly property var confirmed: pkg?.confirmed ?? []
    readonly property var facets: pkg?.facets ?? []
    readonly property var exposes: pkg?.exposes ?? []
    readonly property var rows: [
        {
            "what": "hide_zone",
            "text": App.tr("Let it see the time zone")
        },
        {
            "what": "hide_language",
            "text": App.tr("Let it see the language")
        },
        {
            "what": "hide_gpu",
            "text": App.tr("Let it see the model of the graphics card")
        },
        {
            "what": "hide_storage",
            "text": App.tr("Let it keep data on this computer"),
            "facet": "storage"
        }
    ]

    visible: pkg?.kind === "web"

    Kirigami.Heading {
        level: 4
        text: App.tr("Privacy")
    }

    PrivacyNotice {
        Layout.fillWidth: true
        exposes: privacy.exposes
    }

    Repeater {
        model: privacy.rows.filter(row => !row.facet || privacy.facets.includes(row.facet))

        delegate: QQC2.Switch {
            required property var modelData

            Layout.fillWidth: true
            text: modelData.text
            checked: !privacy.confirmed.includes(modelData.what)
            onToggled: {
                App.act(checked ? "Unconfirm" : "Confirm", [[privacy.uid], modelData.what], "Library1");
                checked = Qt.binding(() => !privacy.confirmed.includes(modelData.what));
            }
        }
    }

    QQC2.Switch {
        Layout.fillWidth: true
        visible: privacy.facets.includes("network")
        text: App.tr("Let it reach the sites it names (show it first)")
        checked: privacy.exposes.includes("network")
        onToggled: {
            App.act(checked ? "Grant" : "Revoke", [privacy.pkg.id, "network"], "Inputs1");
            checked = Qt.binding(() => privacy.exposes.includes("network"));
        }
    }

    QQC2.Button {
        visible: privacy.facets.includes("storage")
        text: App.tr("Clear data")
        icon.name: "edit-clear-history"
        onClicked: App.act("ClearData", [[privacy.uid]], "Library1")
    }
}
