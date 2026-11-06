import QtQuick
import QtQuick.Controls as QQC2

QQC2.Dial {
    id: dial

    property var editor: null
    readonly property SceneHold holding: SceneHold {
        item: dial
        editor: dial.editor
    }

    function hold(on) {
        holding.hold(on);
    }

    onPressedChanged: hold(pressed)
}
