import QtQuick

Item {
    id: overlays

    required property var page
    required property real ratio

    SceneWind {
        anchors.fill: parent
        z: 4
        page: overlays.page
        ratio: overlays.ratio
    }

    SceneSwarm {
        anchors.fill: parent
        z: 5
        page: overlays.page
        ratio: overlays.ratio
    }

    SceneLights {
        anchors.fill: parent
        z: 5
        page: overlays.page
        ratio: overlays.ratio
    }

    Repeater {
        model: (overlays.page.view.gizmos ?? []).length

        delegate: SceneGizmo {
            page: overlays.page
            ratio: overlays.ratio
        }
    }
}
