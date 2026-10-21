/*
 * SPDX-License-Identifier: GPL-3.0-or-later
 *
 * Settings page shown in "Configure Desktop and Wallpaper".
 */
import QtQuick
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.FormLayout {
    id: page

    property string cfg_Image
    property string cfg_FillMode
    property string cfg_ImageDefault
    property string cfg_FillModeDefault

    readonly property var fillModes: [
        { value: "fill", label: i18n("Fill (crop to cover the screen)") },
        { value: "fit", label: i18n("Fit (keep the whole image)") },
        { value: "stretch", label: i18n("Stretch") },
        { value: "center", label: i18n("Center (original size)") },
        { value: "tile", label: i18n("Tile") }
    ]

    RowLayout {
        Kirigami.FormData.label: i18n("Image:")
        QQC2.TextField {
            Layout.minimumWidth: Kirigami.Units.gridUnit * 18
            text: page.cfg_Image
            placeholderText: i18n("Use the daemon setting")
            onTextEdited: page.cfg_Image = text
        }
        QQC2.Button {
            icon.name: "document-open"
            text: i18n("Choose…")
            onClicked: fileDialog.open()
        }
    }

    QQC2.ComboBox {
        Kirigami.FormData.label: i18n("Placement:")
        model: page.fillModes
        textRole: "label"
        valueRole: "value"
        currentIndex: Math.max(0, page.fillModes.findIndex(m => m.value === page.cfg_FillMode))
        onActivated: page.cfg_FillMode = currentValue
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 24
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: i18n("Rendering is done by the wallpaper-rs daemon (wprsd), which must be running in your session.")
    }

    FileDialog {
        id: fileDialog
        title: i18n("Choose an image")
        nameFilters: [i18n("Images (*.png *.jpg *.jpeg *.webp *.gif *.bmp)"), i18n("All files (*)")]
        onAccepted: {
            const url = selectedFile.toString();
            page.cfg_Image = decodeURIComponent(url.startsWith("file://") ? url.slice(7) : url);
        }
    }
}
