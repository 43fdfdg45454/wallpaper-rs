import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: chips

    required property var field
    required property string current
    readonly property var chosen: current.split(",").filter(item => item !== "")
    readonly property var shown: field.values.concat(chosen.filter(item => !field.values.includes(item)))

    signal picked(string value)

    function toggle(item) {
        picked(App.backend.toggle(current, item, field.values.join(",")));
    }

    Flow {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 24
        spacing: Kirigami.Units.smallSpacing

        Repeater {
            model: chips.shown

            delegate: QQC2.Button {
                required property string modelData

                text: App.value(modelData)
                checkable: true
                checked: chips.chosen.includes(modelData)
                onClicked: chips.toggle(modelData)
            }
        }
    }

    QQC2.TextField {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 14
        placeholderText: App.tr("Add another…")
        onAccepted: {
            chips.toggle(text);
            clear();
        }
    }
}
