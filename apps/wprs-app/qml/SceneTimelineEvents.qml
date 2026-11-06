import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: events

    required property var panel
    readonly property var list: panel.line?.events ?? []
    property int chosen: -1
    readonly property var picked: list[chosen] ?? null

    function at(x) {
        return Math.round(Math.max(0, Math.min(1, x / lane.width)) * panel.length * 1000) / 1000;
    }

    function fresh() {
        let n = list.length + 1;
        while (list.some(e => e.name === "event_" + n))
            n++;
        return "event_" + n;
    }

    function moved(e, t, name) {
        panel.put({
                "drop_events": [e.name + "@" + e.t],
                "add_events": [{
                        "t": t,
                        "name": name
                    }]
            });
    }

    Rectangle {
        id: lane

        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 2
        color: Kirigami.Theme.alternateBackgroundColor
        border.color: Kirigami.Theme.disabledTextColor

        TapHandler {
            onDoubleTapped: point => {
                events.panel.put({
                        "add_events": [{
                                "t": events.at(point.position.x),
                                "name": events.fresh()
                            }]
                    });
                events.chosen = -1;
            }
        }

        SceneRepeater {
            id: listed

            rows: events.list

            delegate: Rectangle {
                id: mark

                readonly property var modelData: listed.rows[index]
                required property int index

                x: lane.width * Math.min(1, modelData.t / Math.max(1e-6, events.panel.length)) - width / 2
                y: 2
                width: Kirigami.Units.smallSpacing * 3
                height: lane.height - 4
                radius: width / 2
                color: events.chosen === index ? Kirigami.Theme.highlightColor : Kirigami.Theme.neutralTextColor

                QQC2.ToolTip.text: App.tr("%1 at %2 s: drag to move it", modelData.name, modelData.t)
                QQC2.ToolTip.visible: hover.hovered

                HoverHandler {
                    id: hover
                }

                TapHandler {
                    onTapped: events.chosen = mark.index
                }

                DragHandler {
                    property real last: -1

                    target: null
                    onCentroidChanged: {
                        if (active)
                            last = mark.mapToItem(lane, centroid.position.x, 0).x;
                    }
                    onActiveChanged: {
                        if (active) {
                            events.chosen = mark.index;
                            last = -1;
                            return;
                        }
                        const t = last < 0 ? mark.modelData.t : events.at(last);
                        if (t !== mark.modelData.t)
                            events.moved(mark.modelData, t, mark.modelData.name);
                    }
                }
            }
        }
    }

    RowLayout {
        visible: events.picked !== null

        QQC2.TextField {
            id: named

            text: events.picked?.name ?? ""
            validator: RegularExpressionValidator {
                regularExpression: /[a-z0-9_]{1,32}/
            }
            onEditingFinished: {
                if (events.picked && text !== events.picked.name)
                    events.moved(events.picked, events.picked.t, text);
            }
        }

        QQC2.Label {
            text: App.tr("at %1 s", events.picked?.t ?? 0)
        }

        QQC2.Button {
            icon.name: "edit-delete"
            text: App.tr("Remove")
            onClicked: {
                events.panel.put({
                        "drop_events": [events.picked.name + "@" + events.picked.t]
                    });
                events.chosen = -1;
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: App.tr("Double-click the lane to add an event there; triggers and scripts start on its name.")
    }
}
