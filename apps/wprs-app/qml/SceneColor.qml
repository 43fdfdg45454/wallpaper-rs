import QtQuick
import QtQuick.Dialogs
import QtQuick.Controls as QQC2

QQC2.Button {
    id: swatch

    property string hex: "#ffffff"
    property bool alpha: true

    signal picked(string hex)

    text: hex
    icon.name: "color-picker"
    icon.color: hex.length === 9 ? "#" + hex.slice(7) + hex.slice(1, 7) : hex
    onClicked: dialog.open()

    ColorDialog {
        id: dialog

        selectedColor: swatch.icon.color
        options: swatch.alpha ? ColorDialog.ShowAlphaChannel : 0
        onAccepted: swatch.picked("#" + [selectedColor.r, selectedColor.g, selectedColor.b].concat(swatch.alpha ? [selectedColor.a] : []).map(v => Math.round(v * 255).toString(16).padStart(2, "0")).join("").replace(/^(#[0-9a-f]{6})ff$/, "$1"))
    }
}
