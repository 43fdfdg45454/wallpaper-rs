import QtQuick
import org.kde.kirigami as Kirigami

Rectangle {
    required property var size
    required property real ratio
    required property string aspect
    readonly property var box: {
        if (aspect === "")
            return [0, 0];
        const parts = aspect.split(":").map(Number);
        const wide = parts[0] / parts[1], own = size[0] / size[1];
        return wide > own ? [size[0], size[0] / wide] : [size[1] * wide, size[1]];
    }

    visible: aspect !== ""
    width: box[0] * ratio
    height: box[1] * ratio
    color: "transparent"
    border.color: Kirigami.Theme.highlightColor
    border.width: 2
}
