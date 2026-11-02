import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: browse

    required property Installer installer
    property var packages: []
    property int total
    property var previews: ({})
    property var moving: ({})
    property var asked: ({})
    property var shown: ({})
    property var votes: ({})
    property bool counted
    readonly property bool battery: App.state.on_battery ?? false

    function animate(pkg) {
        const wanted = browse.spec(pkg);
        if (battery || !pkg.animated || pkg.photosensitive === true || browse.asked[wanted])
            return;
        browse.asked[wanted] = true;
        App.query("RequestAnimatedPreviews", [[wanted]], "Repos1", found => browse && (browse.moving = Object.assign({}, browse.moving, found[0])));
    }

    function spec(pkg) {
        return pkg.repo_id + "/" + pkg.id + "@" + pkg.version;
    }

    function search() {
        App.query("Browse", [field.text, 0, 200], "Repos1", reply => {
            if (!browse)
                return;
            browse.packages = reply.packages.filter(p => p.featured).concat(reply.packages.filter(p => !p.featured));
            browse.total = reply.total;
            App.query("GetVotes", [reply.packages.slice(0, 100).map(p => p.id)], "Sync1", found => browse && (browse.votes = found.items ?? {}, browse.counted = found.shown === true), () => browse && (browse.counted = false));
            App.query("RequestPreviews", [reply.packages.filter(p => p.preview).map(browse.spec)], "Repos1", found => browse && (browse.previews = Object.assign({}, browse.previews, found[0])));
        });
    }

    Component.onCompleted: search()

    Connections {
        target: App.backend

        function onPreviewReady(spec, path) {
            if (spec.startsWith("animated:"))
                browse.moving = Object.assign({}, browse.moving, {
                    [spec.slice(9)]: path
                });
            else
                browse.previews = Object.assign({}, browse.previews, {
                    [spec]: path
                });
        }

        function onChanged(topic) {
            if (topic === "repos")
                browse.search();
        }
    }

    Kirigami.SearchField {
        id: field

        Layout.fillWidth: true
        autoAccept: false
        onAccepted: browse.search()
    }

    QQC2.Label {
        visible: browse.total > browse.packages.length
        text: App.tr("Showing %1 of %2 packages; refine the search to see others.", browse.packages.length, browse.total)
    }

    QQC2.ScrollView {
        Layout.fillWidth: true
        Layout.fillHeight: true

        ListView {
            model: browse.packages
            spacing: Kirigami.Units.largeSpacing

            delegate: ColumnLayout {
                id: item

                required property var modelData
                required property int index
                readonly property bool starts: item.index === 0 ? item.modelData.featured === true : !item.modelData.featured && browse.packages[item.index - 1].featured === true

                width: ListView.view.width

                Kirigami.Heading {
                    visible: item.starts
                    level: 3
                    textFormat: Text.PlainText
                    text: item.modelData.featured ? App.tr("Featured by the repositories") : App.tr("All packages")
                }

                BrowseRow {
                    Layout.fillWidth: true
                    modelData: item.modelData
                    catalog: browse
                }
            }
        }
    }
}
