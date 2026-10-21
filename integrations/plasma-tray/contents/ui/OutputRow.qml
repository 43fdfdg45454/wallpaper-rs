import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2

RowLayout {
    id: row

    required property var daemon
    required property string name
    required property var info
    readonly property var fits: ["fill", "fit", "stretch", "center", "tile"]
    readonly property string file: info.image ? info.image.split("/").pop() : i18n("background color")
    readonly property string status: info.error ? info.error : info.kind === "video" ? (info.playing ? i18n("playing") : i18n("paused")) : ""

    QQC2.Label {
        Layout.fillWidth: true
        elide: Text.ElideMiddle
        text: row.name === "" ? i18n("Every screen") : row.name + ": " + row.file + (row.status ? " (" + row.status + ")" : "")
    }

    QQC2.ComboBox {
        visible: row.name !== ""
        enabled: !!row.info.image
        model: row.fits
        currentIndex: row.fits.indexOf(row.info.fit ?? "fill")
        onActivated: row.daemon.act("SetWallpaper", [row.info.image, row.name, currentText])
    }

    QQC2.Button {
        icon.name: "document-open"
        text: i18n("Choose…")
        onClicked: dialog.open()
    }

    FileDialog {
        id: dialog

        nameFilters: [i18n("Images and videos") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.mp4 *.m4v *.webm *.mkv *.mov *.avi *.ogv *.mpg *.mpeg)"]
        onAccepted: row.daemon.act("SetWallpaper", [decodeURIComponent(selectedFile.toString().replace("file://", "")), row.name, ""])
    }
}
