import QtQuick
import QtQuick.Controls as QQC2

QQC2.RangeSlider {
    id: range

    property var editor: null
    readonly property bool held: first.pressed || second.pressed
    readonly property SceneHold holding: SceneHold {
        item: range
        editor: range.editor
    }

    function owner() {
        return holding.owner();
    }

    function hold(on) {
        holding.hold(on);
    }

    onHeldChanged: hold(held)
}
