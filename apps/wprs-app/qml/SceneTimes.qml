import QtQuick
import QtQuick.Layouts

StackLayout {
    required property var page

    currentIndex: page.mesh.clip !== "" ? 1 : 0

    SceneTimeline {
        page: parent.page
    }

    SceneClipLine {
        page: parent.page
    }
}
