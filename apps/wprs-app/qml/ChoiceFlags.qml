import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

RowLayout {
    id: flags

    required property var field
    required property string current

    signal picked(string value)

    Repeater {
        model: flags.field.values

        delegate: QQC2.CheckBox {
            required property string modelData
            required property int index

            text: Labels.field(flags.field.key + "." + modelData)
            checked: flags.current[index] === "1"
            onToggled: flags.picked(App.backend.flip(flags.current, index))
        }
    }
}
