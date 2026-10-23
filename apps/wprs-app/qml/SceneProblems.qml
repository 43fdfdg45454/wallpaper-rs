import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: panel

    required property var page
    readonly property var problems: page.view.problems ?? []

    Kirigami.Heading {
        level: 4
        text: panel.problems.length ? App.tr("Problems (%1)").arg(panel.problems.length) : App.tr("No problems")
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            model: panel.problems
            clip: true

            delegate: QQC2.ItemDelegate {
                required property var modelData

                width: ListView.view.width
                text: modelData.message
                icon.name: modelData.warning ? "data-warning" : "data-error"
                onClicked: panel.page.jump(modelData.pointer)
            }
        }
    }
}
