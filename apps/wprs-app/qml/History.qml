import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: history

    required property var choices
    property var entries: []

    function load() {
        if (output.currentValue)
            App.query("History", [output.currentValue, 100], "Playlists1", reply => history.entries = reply.entries);
    }

    Component.onCompleted: load()

    QQC2.ComboBox {
        id: output

        model: history.choices
        textRole: "text"
        valueRole: "value"
        onActivated: history.load()
    }

    ListView {
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        model: history.entries

        delegate: QQC2.Label {
            required property var modelData

            text: App.time(modelData.at) + " · " + (modelData.item?.title ?? App.tr("an item that was removed"))
        }
    }
}
