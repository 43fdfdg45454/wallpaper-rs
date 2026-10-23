import QtQuick
import QtQuick.Dialogs
import QtQuick.Controls as QQC2

QQC2.Button {
    id: swatch

    property string hex: "#ffffff"

    signal picked(string hex)

    text: hex
    icon.name: "color-picker"
    icon.color: hex.length === 9 ? "#" + hex.slice(7) + hex.slice(1, 7) : hex
    onClicked: dialog.open()

    ColorDialog {
        id: dialog

        selectedColor: swatch.icon.color
        options: ColorDialog.ShowAlphaChannel
        onAccepted: swatch.picked("#" + [selectedColor.r, selectedColor.g, selectedColor.b, selectedColor.a].map(v => Math.round(v * 255).toString(16).padStart(2, "0")).join("").replace(/ff$/, ""))
    }
}
