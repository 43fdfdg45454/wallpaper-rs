import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function exercise() {
        const steps = page.view.history.done.length;
        page.run("timeline", {
                "id": "panel_check",
                "add": true,
                "duration": 4
            });
        const pointer = page.view.timelines.find(t => t.id === "panel_check").pointer;
        const panel = Qt.createComponent("SceneTimelinePanel.qml").createObject(page, {
                "page": page,
                "pointer": pointer
            });
        panel.put({
                "loop": "pingpong",
                "speed": -2,
                "delay": 0.5,
                "autoplay": false,
                "trigger": panel.triggers[0] ?? ""
            });
        panel.put({
                "add_events": [{
                        "t": 1,
                        "name": "spark"
                    }]
            });
        const line = read(pointer);
        if (line.loop !== "pingpong" || line.speed !== -2 || line.delay !== 0.5 || line.autoplay !== false || panel.length !== 4)
            throw new Error("timeline panel: the timeline did not change: " + JSON.stringify(line) + page.editor.error);
        if ((line.events ?? []).length !== 1 || line.events[0].t !== 1 || line.events[0].name !== "spark")
            throw new Error("timeline panel: no event was added: " + JSON.stringify(line.events));
        panel.put({
                "drop_events": ["spark@1"],
                "add_events": [{
                        "t": 2.5,
                        "name": "spark"
                    }]
            });
        if (read(pointer + "/events/0/t") !== 2.5)
            throw new Error("timeline panel: the event did not move");
        const lane = Qt.createComponent("SceneTimelineEvents.qml").createObject(page, {
                "panel": panel
            });
        lane.moved(lane.list[0], 3, "boom");
        if (read(pointer + "/events/0/name") !== "boom" || read(pointer + "/events/0/t") !== 3 || lane.fresh() !== "event_2")
            throw new Error("timeline panel: the lane did not rename or move the event: " + JSON.stringify(read(pointer + "/events")));
        lane.destroy();
        panel.destroy();
        const menu = Qt.createComponent("SceneTimelineMenu.qml").createObject(page, {
                "timeline": {
                    "page": page
                }
            });
        menu.popup();
        const lines = (page.view.timelines ?? []).length;
        if (menu.count !== lines + 1 || menu.menuAt(0) === null)
            throw new Error("timeline menu: it shows " + menu.count + " entries for " + lines + " timelines");
        menu.itemAt(menu.count - 1).triggered();
        if ((page.view.timelines ?? []).length !== lines + 1)
            throw new Error("timeline menu: Add a timeline added none");
        menu.close();
        menu.destroy();
        while (page.view.history.done.length > steps)
            page.run("undo");
    }
}
