import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: pick

    required property var names
    property var chosen: []
    property string label
    property bool single
    signal picked(var list)

    function toggle(name) {
        const has = chosen.includes(name);
        picked(single ? [name] : has ? chosen.filter(n => n !== name) : chosen.concat([name]));
    }

    QQC2.Label {
        text: pick.label + (pick.chosen.length === 0 && !pick.single ? " " + App.tr("(all)") : "")
        font: Kirigami.Theme.smallFont
        visible: pick.label !== ""
    }

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        Repeater {
            model: pick.names

            delegate: QQC2.Button {
                required property string modelData

                text: modelData
                checkable: true
                checked: pick.chosen.includes(modelData)
                font: Kirigami.Theme.smallFont
                onClicked: pick.toggle(modelData)
            }
        }
    }

    QQC2.Button {
        visible: !pick.single && pick.chosen.length > 0
        text: App.tr("All bones")
        flat: true
        onClicked: pick.picked([])
    }
}
