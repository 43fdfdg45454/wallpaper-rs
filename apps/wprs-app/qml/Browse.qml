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
    property var shown: ({})
    readonly property bool battery: App.state.on_battery ?? false

    onBatteryChanged: {
        if (!battery)
            animate(packages);
    }

    function animate(list) {
        const wanted = list.filter(p => p.animated && p.photosensitive !== true).map(browse.spec);
        if (!battery && wanted.length > 0)
            App.query("RequestAnimatedPreviews", [wanted], "Repos1", found => browse && (browse.moving = Object.assign({}, browse.moving, found[0])));
    }

    function spec(pkg) {
        return pkg.repo_id + "/" + pkg.id + "@" + pkg.version;
    }

    function search() {
        App.query("Browse", [field.text, 0, 200], "Repos1", reply => {
            if (!browse)
                return;
            browse.packages = reply.packages;
            browse.total = reply.total;
            App.query("RequestPreviews", [reply.packages.filter(p => p.preview).map(browse.spec)], "Repos1", found => browse && (browse.previews = Object.assign({}, browse.previews, found[0])));
            browse.animate(reply.packages);
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

            delegate: RowLayout {
                id: row

                required property var modelData
                readonly property bool veiled: modelData.content === "mature" && !modelData.revealed && !browse.shown[modelData.repo_id + "/" + modelData.id] && !App.unveiled

                width: ListView.view.width

                Image {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 5
                    source: App.url(browse.previews[browse.spec(row.modelData)] ?? "")
                    asynchronous: true
                    fillMode: Image.PreserveAspectCrop
                    sourceSize: row.veiled ? Qt.size(12, 12) : undefined

                    AnimatedImage {
                        anchors.fill: parent
                        visible: !row.veiled && status === AnimatedImage.Ready
                        source: row.veiled || row.modelData.photosensitive === true ? "" : App.url(browse.moving[browse.spec(row.modelData)] ?? "")
                        playing: visible && !browse.battery && browse.Window.visibility !== Window.Hidden
                        cache: false
                        fillMode: Image.PreserveAspectCrop
                    }

                    TapHandler {
                        enabled: row.veiled
                        onTapped: {
                            const key = row.modelData.repo_id + "/" + row.modelData.id;
                            browse.shown = Object.assign({}, browse.shown, {
                                [key]: true
                            });
                            App.act("Reveal", [key], "Repos1");
                        }
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true

                    Kirigami.Heading {
                        level: 4
                        text: row.modelData.title + " · " + Labels.value(row.modelData.kind)
                        textFormat: Text.PlainText
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        textFormat: Text.PlainText
                        text: [row.modelData.content ? App.plain(row.modelData.description) : row.modelData.description, row.modelData.authors.join(", ") + " · " + row.modelData.license + " · " + App.bytes(row.modelData.size), row.modelData.id + " · " + row.modelData.repo_name, row.modelData.installed.length > 0 ? App.tr("Installed: %1", row.modelData.installed.join(", ")) : "", row.modelData.update && row.modelData.changes ? App.tr("What's new: %1", App.plain(row.modelData.changes)) : "", row.veiled ? App.tr("Mature content: click the picture to show it") : ""].filter(t => t).join("\n")
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        visible: row.modelData.photosensitive === true
                        wrapMode: Text.Wrap
                        color: Kirigami.Theme.negativeTextColor
                        text: App.tr("Flashes or shows fast patterns that can trigger seizures in people with photosensitive epilepsy.")
                    }
                }

                QQC2.ComboBox {
                    id: version

                    model: row.modelData.versions
                    currentIndex: count - 1
                }

                QQC2.Button {
                    text: row.modelData.update ? App.tr("Update") : App.tr("Install")
                    icon.name: "download"
                    onClicked: browse.installer.install(row.modelData.repo_id + "/" + row.modelData.id + "@" + version.currentText)
                }
            }
        }
    }
}
