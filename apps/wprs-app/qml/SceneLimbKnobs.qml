import QtQuick

Item {
    id: knobs

    required property var host

    SceneIkKnobs {
        anchors.fill: parent
        host: knobs.host
    }

    SceneBodyKnobs {
        anchors.fill: parent
        host: knobs.host
        visible: knobs.host.mode === "bodies"
    }

    SceneAttachKnobs {
        anchors.fill: parent
        host: knobs.host
        visible: knobs.host.mode === "attach"
    }
}
