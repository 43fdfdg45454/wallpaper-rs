import QtQuick
import QtQuick.Layouts

Item {
    id: times

    required property var page

    MouseArea {
        anchors.fill: parent
    }

    StackLayout {
        anchors.fill: parent
        currentIndex: times.page.mesh.clip !== "" ? 1 : 0

        SceneTimeline {
            page: times.page
        }

        SceneClipLine {
            page: times.page
        }
    }
}
