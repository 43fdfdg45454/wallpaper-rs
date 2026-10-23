import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Page {
    id: page

    property string query
    readonly property int cell: Kirigami.Units.gridUnit * 10

    function search() {
        library.search(filters.text, filters.kind, filters.favorite, filters.tag);
    }

    function look() {
        const columns = Math.max(1, Math.floor(grid.width / page.cell));
        const top = Math.max(0, Math.floor(grid.contentY / page.cell));
        const bottom = Math.ceil((grid.contentY + grid.height) / page.cell);
        library.visible(top * columns, Math.min(grid.count, bottom * columns) - 1);
    }

    function exercise() {
        tools.exercise(library.uid(0));
    }

    title: App.tr("Library")
    padding: 0
    Component.onCompleted: filters.start(query)
    actions: tools.actions

    header: Filters {
        id: filters

        onSearch: page.search()
        onRename: name => tools.renameTag(name)
    }

    QQC2.ScrollView {
        anchors.fill: parent

        GridView {
            id: grid

            cellWidth: page.cell
            cellHeight: page.cell
            reuseItems: true
            cacheBuffer: page.cell * 2
            model: ItemModel {
                id: library

                size: grid.Screen.devicePixelRatio > 1 ? 512 : 256
            }
            delegate: Tile {
                width: page.cell
                height: page.cell
                onPicked: only => library.toggle(index, only)
                onOpened: tools.inspect(uid)
            }
            onContentYChanged: page.look()
            onHeightChanged: page.look()
            onWidthChanged: page.look()
            onCountChanged: page.look()

            Kirigami.PlaceholderMessage {
                anchors.centerIn: parent
                width: parent.width - Kirigami.Units.gridUnit * 4
                visible: grid.count === 0 && !library.loading
                icon.name: library.error ? "data-error" : "folder-pictures"
                text: library.error || App.tr("Nothing here yet")
                explanation: library.error ? "" : App.tr("Add folders in the settings, import files or drop them here.")
            }
        }
    }

    DropArea {
        anchors.fill: parent
        keys: ["text/uri-list"]
        onDropped: drop => {
            const paths = drop.urls.map(url => App.path(url)).filter(p => p);
            if (paths.length > 0)
                App.act("Import", [paths, false], "Library1");
        }
    }

    Actions {
        id: tools

        items: library
    }
}
