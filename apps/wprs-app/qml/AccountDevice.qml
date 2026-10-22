import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

RowLayout {
    id: row

    required property var device
    required property bool keyed
    required property bool joining
    property string code

    signal share

    QQC2.Label {
        Layout.fillWidth: true
        elide: Text.ElideRight
        text: [row.device.label ?? row.device.name, row.device.label ? row.device.name : "", row.device.current ? App.tr("this computer") : App.tr("seen %1").arg(App.time(row.device.seen_at) || "–")].filter(t => t).join(" · ")
    }

    QQC2.Label {
        visible: row.code !== ""
        text: App.tr("Compare on that device: %1").arg(row.code)
    }

    QQC2.Button {
        visible: row.joining && !row.device.current
        text: App.tr("Get the key from it")
        icon.name: "document-encrypt"
        onClicked: App.query("Join", [row.device.id], "Sync1", reply => row.code = reply[0])
    }

    QQC2.Button {
        visible: row.keyed && !row.device.current
        text: App.tr("Share the key…")
        icon.name: "document-send"
        onClicked: row.share()
    }
}
