import QtQuick
import io.github.wallpaper_rs.app

Timer {
    required property var host
    property int step: 0
    property var opened: null
    property bool edited: false

    function keeps(page) {
        const shown = host.pageStack.currentItem;
        host.activate(page);
        if (host.pageStack.currentItem !== shown)
            throw new Error("a request for the " + page + " page replaced the page shown");
    }

    interval: 300
    repeat: true
    running: App.checking
    onTriggered: {
        const at = step++;
        if (at > 0 && at <= host.pages.length)
            host.pageStack.currentItem.exercise?.();
        if (at < host.pages.length) {
            host.showPage(at);
            keeps(host.names[at]);
        } else if (at === host.pages.length) {
            return;
        } else if (at === host.pages.length + 1) {
            return App.query("GetItem", [""], "Library1", reply => {
                if (!host)
                    return;
                App.edit(reply.item);
                opened = host.pageStack.currentItem;
            });
        } else if (opened?.editor?.ready !== true || host.pageStack.currentItem !== opened) {
            if (at < host.pages.length + 51)
                return;
            stop();
            Qt.quit();
            throw new Error("the image editor did not open");
        } else if (!edited) {
            edited = true;
        } else {
            stop();
            Qt.quit();
            if (opened.editor.dirty !== true)
                throw new Error("the editor has no unsaved edits to keep");
            return keeps("settings");
        }
        if (at > host.pages.length + 1)
            host.pageStack.currentItem.exercise?.();
    }
}
