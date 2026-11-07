import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: view

    required property var page

    QQC2.Switch {
        text: App.tr("Show the depth map over the picture")
        checked: view.page.overlayOn
        onToggled: view.page.overlayOn = checked
    }

    EditSlider {
        text: App.tr("Opacity")
        enabled: view.page.overlayOn
        from: 0.05
        value: view.page.overlayOpacity
        onMoved: value => view.page.overlayOpacity = value
    }

    QQC2.CheckBox {
        text: App.tr("False colour, red near and blue far")
        enabled: view.page.overlayOn
        checked: view.page.overlayColour
        onToggled: view.page.overlayColour = checked
    }
}
