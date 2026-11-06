import QtQuick
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

QQC2.ToolButton {
    required property var page

    icon.name: "document-replace"
    text: App.tr("Write over keyframes")
    display: QQC2.AbstractButton.IconOnly
    checkable: true
    checked: page.overwrite
    onToggled: page.overwrite = checked
    QQC2.ToolTip.text: checked ? App.tr("Handles on the canvas write a fixed value over keyframes and links") : App.tr("Handles on the canvas add a keyframe at the playhead to animated values and leave linked values alone")
    QQC2.ToolTip.visible: hovered
}
