import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: kin

    required property var host
    readonly property var pkg: host.pkg
    readonly property var facets: pkg?.facets ?? []
    readonly property var platforms: ["desktop", "android"].filter(p => facets.includes(p))
    readonly property var base: host.details.base ?? null
    readonly property var variants: host.details.variants ?? []
    readonly property string repo: host.details.installed?.repo_id ?? ""

    visible: !!pkg

    QQC2.Label {
        Layout.fillWidth: true
        text: kin.platforms.length ? App.tr("Works fully on: %1", kin.platforms.map(Labels.value).join(", ")) : App.tr("It does not work fully on any device yet")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: kin.facets.includes("compute") && App.state.compute === false
        type: Kirigami.MessageType.Warning
        text: App.tr("It needs compute shaders, which the graphics of this computer lack or run in software, so it may draw slowly or leave parts out.")
    }

    RowLayout {
        Layout.fillWidth: true
        visible: !!kin.pkg?.base

        QQC2.Label {
            Layout.fillWidth: true
            text: kin.base ? App.tr("Preset of %1", kin.base.name) : App.tr("Preset of %1, which is not installed", kin.pkg?.base ?? "")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
        }

        QQC2.Button {
            visible: !!kin.base
            text: App.tr("Show")
            onClicked: kin.host.show(kin.base.uid)
        }

        QQC2.Button {
            visible: !kin.base && kin.repo !== ""
            text: App.tr("Install it")
            icon.name: "download"
            onClicked: App.act("Install", [kin.repo + "/" + kin.pkg.id + "@" + kin.pkg.version, false, false], "Repos1")
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: kin.variants.length > 0
        text: App.tr("Presets of this wallpaper:")
    }

    Repeater {
        model: kin.variants

        delegate: QQC2.Button {
            required property var modelData

            text: modelData.name
            icon.name: "preferences-desktop-wallpaper"
            onClicked: kin.host.apply(modelData.uid)
        }
    }
}
