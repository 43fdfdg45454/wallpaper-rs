import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

RowLayout {
    id: quality

    required property var page
    property string again: App.tr("Restart")

    signal picked

    QQC2.Label {
        text: App.tr("Preview quality")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("Q0, lowest"), App.tr("Q1"), App.tr("Q2"), App.tr("Q3, highest")]
        currentIndex: SceneLook.levels.indexOf(SceneLook.quality)
        onActivated: index => {
            SceneLook.quality = SceneLook.levels[index];
            SceneLook.preview(quality.page);
            quality.picked();
        }
    }

    QQC2.Button {
        icon.name: "view-refresh"
        text: quality.again
        onClicked: SceneLook.again(quality.page)
    }
}
