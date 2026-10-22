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

    function spec(pkg) {
        return pkg.repo_id + "/" + pkg.id + "@" + pkg.version;
    }

    function search() {
        App.query("Browse", [field.text, 0, 200], "Repos1", reply => {
            browse.packages = reply.packages;
            browse.total = reply.total;
            App.query("RequestPreviews", [reply.packages.filter(p => p.preview).map(browse.spec)], "Repos1", found => browse.previews = Object.assign({}, browse.previews, found[0]));
        });
    }

    Component.onCompleted: search()

    Connections {
        target: App.backend

        function onPreviewReady(spec, path) {
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
        text: App.tr("Showing %1 of %2 packages; refine the search to see others.").arg(browse.packages.length).arg(browse.total)
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

                width: ListView.view.width

                Image {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 8
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 5
                    source: App.url(browse.previews[browse.spec(row.modelData)] ?? "")
                    asynchronous: true
                    fillMode: Image.PreserveAspectCrop
                }

                ColumnLayout {
                    Layout.fillWidth: true

                    Kirigami.Heading {
                        level: 4
                        text: row.modelData.title + " · " + Labels.value(row.modelData.kind)
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        wrapMode: Text.Wrap
                        text: [row.modelData.description, row.modelData.authors.join(", ") + " · " + row.modelData.license + " · " + App.bytes(row.modelData.size), row.modelData.id + " · " + row.modelData.repo_name, row.modelData.installed.length > 0 ? App.tr("Installed: %1").arg(row.modelData.installed.join(", ")) : ""].filter(t => t).join("\n")
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
