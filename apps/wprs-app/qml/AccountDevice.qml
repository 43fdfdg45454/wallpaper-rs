import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

RowLayout {
    id: row

    required property var device
    required property bool keyed
    required property bool joining
    required property var pairing
    readonly property string code: row.pairing && !row.pairing.sharing && row.pairing.device === row.device.id ? (row.pairing.code ?? "") : ""

    signal share

    QQC2.Label {
        Layout.fillWidth: true
        elide: Text.ElideRight
        text: [row.device.label ?? row.device.name, row.device.label ? row.device.name : "", row.device.current ? App.tr("this computer") : App.tr("seen %1").arg(App.time(row.device.seen_at) || "–")].filter(t => t).join(" · ")
    }

    QQC2.Label {
        visible: row.code !== ""
        text: App.tr("This computer shows %1. Type the code that device shows:").arg(row.code)
    }

    QQC2.TextField {
        id: digits

        visible: row.code !== ""
        placeholderText: App.tr("The 6 digits shown on that device")
    }

    QQC2.Button {
        visible: row.code !== ""
        enabled: digits.text.trim() !== ""
        text: App.tr("Take the key")
        icon.name: "dialog-ok-apply"
        onClicked: App.act("ConfirmShare", [row.device.id, digits.text], "Sync1")
    }

    QQC2.Button {
        visible: row.joining && !row.device.current
        text: App.tr("Get the key from it")
        icon.name: "document-encrypt"
        onClicked: App.act("Join", [row.device.id], "Sync1")
    }

    QQC2.Button {
        visible: row.keyed && !row.device.current
        text: App.tr("Share the key…")
        icon.name: "document-send"
        onClicked: row.share()
    }
}
