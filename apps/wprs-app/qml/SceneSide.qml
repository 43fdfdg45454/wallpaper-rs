import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    property alias tabs: side
    property alias settings: settings

    QQC2.TabBar {
        id: side

        Layout.fillWidth: true
        Layout.minimumWidth: 0
        Layout.preferredWidth: 0

        QQC2.TabButton {
            text: App.tr("Layers")
        }

        QQC2.TabButton {
            text: App.tr("Assets")
        }

        QQC2.TabButton {
            text: App.tr("Package")
        }
    }

    StackLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumWidth: 0
        Layout.preferredWidth: 0
        currentIndex: side.currentIndex

        SceneLayers {
            page: panel.page
        }

        SceneAssets {
            page: panel.page
        }

        SceneProperties {
            id: settings

            page: panel.page
        }
    }
}
