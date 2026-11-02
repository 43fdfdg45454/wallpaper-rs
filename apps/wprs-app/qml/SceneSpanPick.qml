import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

RowLayout {
    id: pick

    required property var page
    readonly property string layout: ["", "dual", "triple", "vertical", "mine"][spans.currentIndex]

    QQC2.Label {
        text: App.tr("Screens:")
    }

    QQC2.ComboBox {
        id: spans

        model: [App.tr("One screen"), App.tr("Two side by side"), App.tr("Three side by side"), App.tr("Two stacked"), App.tr("My screens")]
    }

    QQC2.CheckBox {
        visible: spans.currentIndex > 0
        text: App.tr("Spread over every screen")
        checked: pick.page.editor.value("/manifest/display/span") === "\"all-outputs\""
        onToggled: checked ? pick.page.change("/manifest/display/span", "all-outputs") : pick.page.run("remove", {
            "pointer": "/manifest/display/span"
        })
    }
}
