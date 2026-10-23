import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: side

    required property var page
    property alias currentIndex: pick.currentIndex

    spacing: 0

    ListView {
        id: pick

        Layout.fillWidth: true
        Layout.preferredHeight: contentHeight
        model: side.page.tools
        interactive: false
        activeFocusOnTab: true
        keyNavigationEnabled: true
        Accessible.role: Accessible.PageTabList

        delegate: QQC2.ItemDelegate {
            required property var modelData
            required property int index

            width: ListView.view.width
            text: modelData[0]
            icon.name: modelData[1]
            highlighted: ListView.isCurrentItem
            Accessible.role: Accessible.PageTab
            onClicked: {
                pick.currentIndex = index;
                pick.forceActiveFocus();
            }
        }
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    QQC2.ScrollView {
        id: list

        Layout.fillWidth: true
        Layout.fillHeight: true
        contentWidth: availableWidth

        EditOps {
            x: Kirigami.Units.smallSpacing
            width: list.availableWidth - 2 * Kirigami.Units.smallSpacing
            page: side.page
        }
    }
}
