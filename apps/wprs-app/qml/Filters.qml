import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: filters

    property string kind
    property string tag
    property bool favorite
    property var tags: []
    readonly property alias text: field.text

    signal search
    signal rename(string name)

    function load() {
        App.query("ListTags", [], "Library1", reply => filters.tags = reply.tags);
    }

    function start(query) {
        field.text = query;
        search();
        debounce.stop();
        load();
    }

    onKindChanged: search()
    onFavoriteChanged: search()
    onTagChanged: search()

    Kirigami.SearchField {
        id: field

        Layout.fillWidth: true
        autoAccept: false
        placeholderText: App.tr("Search, for example: tag:sea kind:image -fav:yes sort:random")
        onTextChanged: debounce.restart()
        onAccepted: filters.search()
    }

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        Repeater {
            model: ["image", "video", "package", "scene"]

            delegate: Kirigami.Chip {
                required property string modelData

                text: Labels.value(modelData)
                checkable: true
                closable: false
                checked: filters.kind === modelData
                onClicked: filters.kind = checked ? modelData : ""
            }
        }

        Kirigami.Chip {
            text: App.tr("Favorites")
            icon.name: "emblem-favorite"
            checkable: true
            closable: false
            checked: filters.favorite
            onClicked: filters.favorite = checked
        }

        Repeater {
            model: filters.tags

            delegate: Kirigami.Chip {
                required property var modelData

                text: modelData.name + " (" + modelData.count + ")"
                checkable: true
                closable: false
                checked: filters.tag === modelData.name
                onClicked: filters.tag = checked ? modelData.name : ""
                onPressAndHold: filters.rename(modelData.name)
            }
        }
    }

    Timer {
        id: debounce

        interval: 250
        onTriggered: filters.search()
    }

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (topic === "library")
                filters.load();
        }
    }
}
