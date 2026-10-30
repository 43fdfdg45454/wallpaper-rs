import QtQuick
import QtQuick.Controls as QQC2

QQC2.TextArea {
    id: area

    Component.onCompleted: {
        if (background)
            background.visible = true;
        else
            background = frame.createObject(area);
    }

    Component {
        id: frame

        Rectangle {
            color: "transparent"
            border.width: area.activeFocus ? 2 : 1
            border.color: area.activeFocus ? area.palette.highlight : area.palette.mid
        }
    }
}
