import QtQuick

QtObject {
    id: holding

    required property Item item
    property var editor: null
    property var held: null

    function owner() {
        for (let at = holding.item; at; at = at.parent) {
            if (at.editor?.hold)
                return at.editor;
            if (at.page?.editor?.hold)
                return at.page.editor;
            if (at.tools?.page?.editor?.hold)
                return at.tools.page.editor;
        }
        return null;
    }

    function hold(on) {
        if (on) {
            held = editor ?? owner();
            held?.hold(true);
        } else {
            const was = held ?? editor ?? owner();
            held = null;
            was?.hold(false);
        }
    }

    Component.onDestruction: {
        if (held)
            held.hold(false);
    }
}
