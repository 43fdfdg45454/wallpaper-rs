import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: curve

    required property var timeline
    readonly property var track: timeline.chosen ? timeline.tracks.find(t => t.pointer === timeline.chosen.pointer) ?? null : null
    readonly property var from: timeline.picked
    readonly property var to: track && timeline.chosen ? track.keys[timeline.chosen.index + 1] ?? null : null
    readonly property bool shaped: plot.shaped

    function refit() {
        plot.refit();
    }

    function press(side) {
        plot.press(side);
    }

    function grab(px, py) {
        plot.grab(px, py);
    }

    function commit() {
        plot.commit();
    }

    function send(side, pair) {
        const args = {
            "pointer": track.pointer,
            "time": side === "out" ? from[0] : to[0]
        };
        args[side + "_handle"] = pair;
        timeline.page.run("keyframe", args, true);
    }

    title: App.tr("Curve to the next keyframe")
    standardButtons: Kirigami.Dialog.Close
    onOpened: refit()
    onFromChanged: refit()

    ColumnLayout {
        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 20
            wrapMode: Text.Wrap
            text: !curve.shaped ? App.tr("Handles bend the curve between two keyframes of numbers, lists of numbers or colors; pick one that has another after it.") : plot.scalar ? App.tr("Drag the two handles to bend how the number moves between the keyframes.") : App.tr("Drag the two handles to bend how fast the value goes from one keyframe to the next: the height is the share of the way, from 0 to 1.")
        }

        SceneHandlePlot {
            id: plot

            from: curve.from
            to: curve.to
            onPressed: timeline.page.editor.hold(true)
            onDragged: (side, pair) => curve.send(side, pair)
            onReleased: timeline.page.editor.hold(false)
        }

        SceneKeyBase {
            Layout.fillWidth: true
            curve: curve
        }
    }
}
