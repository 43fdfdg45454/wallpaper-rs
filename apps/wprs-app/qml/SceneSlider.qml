import QtQuick
import QtQuick.Controls as QQC2

QQC2.Slider {
    id: slider

    property var editor: null
    readonly property SceneHold holding: SceneHold {
        item: slider
        editor: slider.editor
    }

    function owner() {
        return holding.owner();
    }

    function hold(on) {
        holding.hold(on);
    }

    onPressedChanged: hold(pressed)
}
