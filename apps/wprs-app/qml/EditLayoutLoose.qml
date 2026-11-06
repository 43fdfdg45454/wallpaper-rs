import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: loose

    required property var bounds
    readonly property var outputs: App.state.outputs ?? {}
    readonly property var names: Object.keys(outputs).filter(name => !outputs[name].screen)
    readonly property var placed: Object.keys(outputs).filter(name => outputs[name].screen)

    function choices(name) {
        const found = [{
                "value": "whole",
                "text": App.tr("The whole wallpaper")
            }, {
                "value": "hand",
                "text": App.tr("Placed by hand on the board")
            }];
        for (const other of Object.keys(outputs).filter(o => o !== name))
            found.push({
                    "value": "follow:" + other,
                    "text": App.tr("The same part as %1", other)
                });
        return found;
    }

    function current(name) {
        const o = outputs[name];
        if (o.follows)
            return "follow:" + o.follows;
        return o.mm ? "hand" : "whole";
    }

    function choose(name, value) {
        const o = outputs[name];
        App.act("SetOption", ["layout.follow." + name, value.startsWith("follow:") ? value.slice(7) : "none"]);
        if (value !== "hand")
            return App.act("SetOption", ["layout.monitors." + name, "none"]);
        const w = Math.max(1, o.width) / 3.78, h = Math.max(1, o.height) / 3.78;
        const at = [bounds[0] + bounds[2] + 10, bounds[1], w, h];
        App.act("SetOption", ["layout.monitors." + name, at.map(v => v.toFixed(1)).join(",")]);
    }

    visible: names.length > 0

    Kirigami.Heading {
        level: 5
        text: App.tr("Outputs that do not say where they are")
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Streams, previews and some screens have no place on the desktop. Choose what part of a spread wallpaper they show; one placed by hand appears on the board above to be dragged.")
    }

    Repeater {
        model: loose.names

        delegate: RowLayout {
            id: row

            required property string modelData
            readonly property var options: loose.choices(modelData)

            QQC2.Label {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                text: row.modelData
                elide: Text.ElideRight
            }

            QQC2.ComboBox {
                Layout.fillWidth: true
                Accessible.name: App.tr("Part shown by %1", row.modelData)
                model: row.options
                textRole: "text"
                valueRole: "value"
                currentIndex: Math.max(0, row.options.findIndex(c => c.value === loose.current(row.modelData)))
                onActivated: loose.choose(row.modelData, currentValue)
            }
        }
    }
}
