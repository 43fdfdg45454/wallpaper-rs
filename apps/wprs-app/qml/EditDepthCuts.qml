import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: cuts

    required property var page
    readonly property var relief: page.relief
    readonly property var survey: JSON.parse(relief.survey || "{}")
    readonly property var manual: page.edit.depth?.cuts
    readonly property var list: page.cutsLive ?? manual ?? survey.cuts ?? []
    readonly property real wide: ruler.width > 0 ? ruler.width : width
    readonly property var hues: ["#4287f5", "#34c759", "#facc15", "#f97316", "#ef4444", "#a855f7"]
    readonly property real gap: 0.01
    property int picked: -1

    function put(list) {
        page.run("depth_cuts", list.length > 0 ? {
            "cuts": list.map(v => v.toFixed(4)).join(",")
        } : {
            "auto": true
        });
    }

    function move(index, x) {
        const now = list.slice();
        const low = index > 0 ? now[index - 1] + gap : gap, high = index < now.length - 1 ? now[index + 1] - gap : 1 - gap;
        now[index] = Math.max(low, Math.min(high, x / wide));
        picked = index;
        page.cutsLive = now;
        relief.trace(page.editor.json, JSON.stringify(now));
    }

    function release() {
        const now = page.cutsLive;
        page.cutsLive = null;
        if (now)
            put(now);
    }

    function add(at) {
        const edges = [0].concat(list, [1]);
        const wide = edges.map((v, i) => i === 0 ? 0 : edges[i] - edges[i - 1]);
        const where = at ?? edges[wide.indexOf(Math.max(...wide)) - 1] + Math.max(...wide) / 2;
        if (list.length < 5 && list.every(v => Math.abs(v - where) >= gap))
            put(list.concat([where]).sort((a, b) => a - b));
    }

    function drop(index) {
        picked = -1;
        put(list.filter((v, i) => i !== index));
    }

    Item {
        id: ruler

        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit * 5

        Canvas {
            anchors.fill: parent
            property var bars: cuts.survey.histogram ?? []
            property var edges: cuts.list

            onBarsChanged: requestPaint()
            onEdgesChanged: requestPaint()
            onPaint: {
                const ctx = getContext("2d");
                ctx.reset();
                const n = bars.length;
                for (let i = 0; i < n; i++) {
                    const at = (i + 0.5) / n, layer = edges.filter(c => c <= at).length;
                    ctx.fillStyle = cuts.hues[layer % cuts.hues.length];
                    ctx.globalAlpha = 0.75;
                    const h = Math.sqrt(bars[i]) * height;
                    ctx.fillRect(i * width / n, height - h, Math.ceil(width / n), h);
                }
            }
        }

        MouseArea {
            anchors.fill: parent
            onClicked: mouse => cuts.add(mouse.x / cuts.wide)
        }

        Repeater {
            model: cuts.list

            delegate: Rectangle {
                id: handle

                required property real modelData
                required property int index

                x: modelData * ruler.width - width / 2
                width: 4
                height: ruler.height
                color: cuts.picked === index ? Kirigami.Theme.highlightColor : Kirigami.Theme.textColor

                Rectangle {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Kirigami.Units.gridUnit
                    height: width
                    radius: width / 2
                    color: parent.color
                    border.color: Kirigami.Theme.backgroundColor
                    border.width: 2
                }

                MouseArea {
                    anchors.fill: parent
                    anchors.margins: -Kirigami.Units.smallSpacing
                    cursorShape: Qt.SplitHCursor
                    onPositionChanged: mouse => {
                        if (pressed)
                            cuts.move(handle.index, mapToItem(ruler, mouse.x, 0).x);
                    }
                    onPressed: cuts.picked = handle.index
                    onReleased: cuts.release()
                    onDoubleClicked: cuts.drop(handle.index)
                }
            }
        }
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Far")
        }

        Item {
            Layout.fillWidth: true
        }

        QQC2.Label {
            text: App.tr("Near")
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: cuts.manual !== undefined ? App.tr("%1 layers, cut where you set the handles", cuts.list.length + 1) : App.tr("%1 layers, cut by the picture itself", cuts.list.length + 1)
        wrapMode: Text.Wrap
    }

    QQC2.Switch {
        text: App.tr("Show the layers on the picture")
        checked: cuts.page.outlineOn
        onToggled: {
            cuts.page.outlineOn = checked;
            if (checked)
                cuts.relief.trace(cuts.page.editor.json, JSON.stringify(cuts.list));
        }
    }

    RowLayout {
        QQC2.Button {
            icon.name: "list-add"
            text: App.tr("Add a cut")
            enabled: cuts.list.length < 5
            onClicked: cuts.add(null)
        }

        QQC2.Button {
            icon.name: "list-remove"
            text: App.tr("Remove the cut")
            enabled: cuts.list.length > 1 || cuts.manual !== undefined
            onClicked: cuts.drop(cuts.picked >= 0 ? cuts.picked : cuts.list.length - 1)
        }

        QQC2.Button {
            icon.name: "view-refresh"
            text: App.tr("Back to automatic")
            enabled: cuts.manual !== undefined
            onClicked: cuts.put([])
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: App.tr("Drag a handle to move a cut, double-click it to remove it, click the strip to add one.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }
}
