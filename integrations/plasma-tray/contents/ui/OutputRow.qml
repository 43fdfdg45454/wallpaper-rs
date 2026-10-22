import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2

RowLayout {
    id: row

    required property var daemon
    required property string name
    required property var info
    readonly property var fits: [
        {
            "value": "fill",
            "text": i18nc("@item:inlistbox wallpaper fit mode", "Fill")
        },
        {
            "value": "fit",
            "text": i18nc("@item:inlistbox wallpaper fit mode", "Fit")
        },
        {
            "value": "stretch",
            "text": i18nc("@item:inlistbox wallpaper fit mode", "Stretch")
        },
        {
            "value": "center",
            "text": i18nc("@item:inlistbox wallpaper fit mode", "Center")
        },
        {
            "value": "tile",
            "text": i18nc("@item:inlistbox wallpaper fit mode", "Tile")
        }
    ]
    readonly property string file: info.image ? info.image.split("/").pop() : i18n("background color")
    readonly property string status: info.error ? info.error : info.kind === "video" ? (info.playing ? i18n("playing") : i18n("paused")) : ""

    QQC2.Label {
        Layout.fillWidth: true
        elide: Text.ElideMiddle
        text: row.name === "" ? i18n("Every screen") : row.name + ": " + row.file + (row.status ? " (" + row.status + ")" : "")
    }

    QQC2.ComboBox {
        enabled: row.name === "" || !!row.info.image
        model: row.fits
        textRole: "text"
        valueRole: "value"
        currentIndex: Math.max(0, row.fits.findIndex(fit => fit.value === (row.info.fit ?? "fill")))
        onActivated: row.daemon.act("SetWallpaper", ["", row.name, currentValue])
    }

    QQC2.Button {
        icon.name: "document-open"
        text: i18n("Choose…")
        onClicked: dialog.open()
    }

    QQC2.ToolButton {
        icon.name: "color-picker"
        text: i18n("Background color")
        display: QQC2.AbstractButton.IconOnly
        QQC2.ToolTip.text: text
        QQC2.ToolTip.visible: hovered
        onClicked: colors.open()
    }

    ColorDialog {
        id: colors

        selectedColor: row.info.background ?? "black"
        onAccepted: row.daemon.act("SetBackground", [selectedColor.toString(), row.name])
    }

    FileDialog {
        id: dialog

        nameFilters: [i18n("Images and videos") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.mp4 *.m4v *.webm *.mkv *.mov *.avi *.ogv *.mpg *.mpeg)"]
        onAccepted: row.daemon.act("SetWallpaper", [decodeURIComponent(selectedFile.toString().replace("file://", "")), row.name, ""])
    }
}
