import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.plasma.workspace.dbus as DBus

ColumnLayout {
    QQC2.Label {
        Layout.fillWidth: true
        text: i18nd("plasma_wallpaper_io.github.wallpaper_rs", "wallpaper-rs is managed from its app, its icon in the system tray or the wprs-ctl command.")
        wrapMode: Text.Wrap
    }

    QQC2.Button {
        icon.name: "preferences-desktop-wallpaper"
        text: i18nd("plasma_wallpaper_io.github.wallpaper_rs", "Open wallpaper-rs")
        onClicked: DBus.SessionBus.asyncCall({
            "service": "io.github.wallpaper_rs",
            "path": "/io/github/wallpaper_rs",
            "iface": "org.freedesktop.Application",
            "member": "Activate",
            "signature": "(a{sv})",
            "arguments": [
                {}
            ]
        }, () => {}, () => {})
    }
}
