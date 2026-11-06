import QtQuick
import QtQuick.Controls as QQC2

QQC2.RangeSlider {
    id: range

    property var editor: null
    readonly property bool held: first.pressed || second.pressed

    function owner() {
        for (let item = range.parent; item; item = item.parent) {
            if (item.editor?.hold)
                return item.editor;
            if (item.page?.editor?.hold)
                return item.page.editor;
            if (item.tools?.page?.editor?.hold)
                return item.tools.page.editor;
        }
        return null;
    }

    function hold(on) {
        (range.editor ?? owner())?.hold(on);
    }

    onHeldChanged: hold(held)
}
