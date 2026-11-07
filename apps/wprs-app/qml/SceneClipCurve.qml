import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: curve

    required property var line
    readonly property var row: line.chosen ? line.rows.find(r => line.same(r, line.chosen.row)) ?? null : null
    readonly property int index: row && line.picked ? row.keys.findIndex(k => Math.abs(k.t - line.picked.t) < 1e-3) : -1
    readonly property var from: index >= 0 ? pair(row.keys[index]) : null
    readonly property var to: index >= 0 && index + 1 < row.keys.length ? pair(row.keys[index + 1]) : null
    readonly property bool shaped: plot.shaped

    function pair(k) {
        return [k.t, k.value, k.ease];
    }

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

    function send(side, handle) {
        const k = row.keys[side === "out" ? index : index + 1];
        const args = {
            "time": k.t
        };
        args[side + "_handle"] = handle;
        line.key(row, Object.assign(args, line.held(row, k)), true);
    }

    title: App.tr("Curve to the next key")
    standardButtons: Kirigami.Dialog.Close
    onOpened: refit()

    ColumnLayout {
        QQC2.Label {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 20
            wrapMode: Text.Wrap
            text: !curve.shaped ? App.tr("Handles bend the curve between two keys of numbers or lists of numbers; pick a key that has another after it.") : plot.scalar ? App.tr("Drag the two handles to bend how the number moves between the keys.") : App.tr("Drag the two handles to bend how fast the value goes from one key to the next: the height is the share of the way, from 0 to 1.")
        }

        SceneHandlePlot {
            id: plot

            from: curve.from
            to: curve.to
            onPressed: curve.line.page.editor.hold(true)
            onDragged: (side, handle) => curve.send(side, handle)
            onReleased: curve.line.page.editor.hold(false)
        }
    }
}
