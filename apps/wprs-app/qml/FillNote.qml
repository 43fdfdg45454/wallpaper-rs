import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: note

    readonly property var lama: JSON.parse(maps.assets || "[]").find(asset => asset.inpaint)
    readonly property bool wanted: (App.state.options ?? {})["parallax.fill"] === "lama"

    visible: wanted && lama !== undefined && !lama.installed

    DepthMaps {
        id: maps

        Component.onCompleted: open("")
    }

    QQC2.Label {
        Layout.fillWidth: true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 28
        text: note.lama ? App.tr("LaMa is not installed, so pictures use the simple fill. It is downloaded once from %1 (%2, license %3), checked against its SHA-256 and runs on this computer; computing the layers of a picture takes a few minutes.", note.lama.hosts, note.lama.size, note.lama.license) : ""
        wrapMode: Text.Wrap
    }

    RowLayout {
        QQC2.Button {
            text: App.tr("Download LaMa")
            icon.name: "download"
            enabled: maps.allowed && !maps.busy
            onClicked: maps.fetchLama()
        }

        QQC2.ProgressBar {
            visible: maps.busy
            from: 0
            to: 1
            indeterminate: maps.progress < 0
            value: maps.progress
        }

        QQC2.Label {
            visible: !maps.allowed
            text: App.tr("Downloads are turned off in the settings.")
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: maps.error !== ""
        text: maps.error
        wrapMode: Text.Wrap
    }
}
