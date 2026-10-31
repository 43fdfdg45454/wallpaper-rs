import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: layout

    readonly property var screens: Object.keys(App.state.outputs ?? {}).filter(name => App.state.outputs[name].mm).map(name => ({
                "name": name,
                "mm": App.state.outputs[name].mm,
                "key": App.state.outputs[name].screen?.identity || name
            }))
    readonly property var bounds: {
        const lo = i => Math.min(...screens.map(s => s.mm[i])), hi = i => Math.max(...screens.map(s => s.mm[i] + s.mm[i + 2]));
        return screens.length ? [lo(0), lo(1), Math.max(1, hi(0) - lo(0)), Math.max(1, hi(1) - lo(1))] : [0, 0, 1, 1];
    }
    readonly property real factor: board.width / bounds[2]

    function place(key, mm) {
        App.act("SetOption", ["layout.monitors." + key, mm.map(v => v.toFixed(1)).join(",")]);
    }

    Component.onDestruction: {
        if (App.state.calibrating)
            App.act("ShowCalibration", [false]);
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Where the screens are")
    }

    Item {
        id: board

        Layout.fillWidth: true
        Layout.preferredHeight: Math.min(Kirigami.Units.gridUnit * 12, width * layout.bounds[3] / layout.bounds[2])
        visible: layout.screens.length > 0

        Repeater {
            model: layout.screens

            delegate: Rectangle {
                id: shown

                required property var modelData
                property var moving: null
                readonly property var mm: moving ?? modelData.mm

                x: (mm[0] - layout.bounds[0]) * layout.factor
                y: (mm[1] - layout.bounds[1]) * layout.factor
                width: mm[2] * layout.factor
                height: mm[3] * layout.factor
                color: Kirigami.Theme.alternateBackgroundColor
                border.color: drag.pressed ? Kirigami.Theme.highlightColor : Kirigami.Theme.textColor
                border.width: 2

                QQC2.Label {
                    anchors.fill: parent
                    anchors.margins: Kirigami.Units.smallSpacing
                    text: shown.modelData.name + "\n" + App.tr("%1 × %2 mm", Math.round(shown.mm[2]), Math.round(shown.mm[3]))
                    horizontalAlignment: Text.AlignHCenter
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                    font: Kirigami.Theme.smallFont
                }

                MouseArea {
                    id: drag

                    property point start

                    anchors.fill: parent
                    cursorShape: Qt.SizeAllCursor
                    onPressed: mouse => start = mapToItem(board, mouse.x, mouse.y)
                    onPositionChanged: mouse => {
                        const at = mapToItem(board, mouse.x, mouse.y), m = shown.modelData.mm;
                        shown.moving = [m[0] + (at.x - start.x) / layout.factor, m[1] + (at.y - start.y) / layout.factor, m[2], m[3]];
                    }
                    onReleased: {
                        if (shown.moving)
                            layout.place(shown.modelData.key, shown.moving);
                        shown.moving = null;
                    }
                }
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: layout.screens.length ? App.tr("Drag a screen to where it really stands; the space between two screens is their frames.") : App.tr("The desktop has not told where its screens are yet.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }

    EditSlider {
        text: App.tr("Width of each screen frame (mm)")
        to: 100
        stepSize: 0.5
        value: App.state.options?.["layout.bezel_mm"] ?? 0
        onMoved: value => App.act("SetOption", ["layout.bezel_mm", String(value)])
    }

    QQC2.Switch {
        Layout.fillWidth: true
        text: App.tr("Show a pattern on the screens to check them")
        checked: App.state.calibrating ?? false
        onToggled: App.act("ShowCalibration", [checked])
    }

    QQC2.Button {
        text: App.tr("Place the screens automatically")
        icon.name: "edit-reset"
        enabled: layout.screens.length > 0
        onClicked: layout.screens.forEach(s => new Set([s.key, s.name]).forEach(key => App.act("SetOption", ["layout.monitors." + key, "none"])))
    }
}
