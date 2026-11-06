import QtQuick
import QtQuick.Controls as QQC2

QQC2.Slider {
    id: slider

    property var editor: null

    function owner() {
        for (let item = slider.parent; item; item = item.parent) {
            if (item.editor?.hold)
                return item.editor;
            if (item.page?.editor?.hold)
                return item.page.editor;
        }
        return null;
    }

    function hold(on) {
        (slider.editor ?? owner())?.hold(on);
    }

    onPressedChanged: hold(pressed)
}
