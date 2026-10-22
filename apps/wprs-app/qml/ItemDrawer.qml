import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.OverlayDrawer {
    id: drawer

    property string uid
    property string preview
    property var details: ({})
    property string repo
    property bool removable
    readonly property var item: details.item ?? {}
    readonly property var credit: details.credit ?? null

    signal apply(string uid)
    signal remove(string uid)

    function show(uid) {
        drawer.uid = uid;
        details = {};
        preview = "";
        repo = "";
        load();
        App.query("Thumbnail", [uid, 1024], "Library1", reply => {
            if (drawer.uid === uid)
                drawer.preview = reply[0];
        });
        open();
    }

    function load() {
        const uid = drawer.uid;
        App.query("GetItem", [uid], "Library1", reply => {
            if (drawer.uid !== uid)
                return;
            drawer.details = reply;
            if (reply.installed)
                App.query("List", [], "Repos1", list => {
                    if (drawer.uid === uid)
                        drawer.repo = list.repos.find(repo => repo.repo_id === reply.installed.repo_id)?.name ?? "";
                });
        });
    }

    edge: Qt.RightEdge
    modal: true
    handleVisible: false
    width: Math.min(Kirigami.Units.gridUnit * 24, parent ? parent.width : 0)

    Connections {
        target: App.backend

        function onChanged(topic) {
            if (drawer.opened && ["library", "playlists", "profiles", "repos"].includes(topic))
                drawer.load();
        }
    }

    contentItem: QQC2.ScrollView {
        ColumnLayout {
            width: drawer.width - Kirigami.Units.largeSpacing * 2

            Image {
                Layout.fillWidth: true
                Layout.preferredHeight: width * 9 / 16
                source: App.url(drawer.preview)
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: false
            }

            QQC2.TextField {
                Layout.fillWidth: true
                text: drawer.item.title ?? ""
                onAccepted: App.act("SetTitle", [drawer.uid, text], "Library1")
            }

            RowLayout {
                Stars {
                    rating: drawer.item.rating ?? 0
                    onRated: stars => App.act("SetRating", [[drawer.uid], stars], "Library1")
                }

                QQC2.ToolButton {
                    icon.name: drawer.item.favorite ? "emblem-favorite" : "bookmark-new"
                    text: drawer.item.favorite ? App.tr("Unfavorite") : App.tr("Favorite")
                    onClicked: App.act("SetFavorite", [[drawer.uid], !drawer.item.favorite], "Library1")
                }
            }

            Kirigami.FormLayout {
                Layout.fillWidth: true

                QQC2.Label {
                    Kirigami.FormData.label: App.tr("Kind:")
                    text: [Labels.value(drawer.item.package?.kind === "scene" ? "scene" : drawer.item.kind), drawer.item.width ? drawer.item.width + "×" + drawer.item.height : ""].filter(t => t).join(" · ")
                }

                QQC2.Label {
                    Kirigami.FormData.label: App.tr("File:")
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 14
                    text: drawer.item.path ?? ""
                    wrapMode: Text.WrapAnywhere
                }

                QQC2.Label {
                    Kirigami.FormData.label: App.tr("Problem:")
                    visible: text !== ""
                    text: drawer.item.missing ? App.tr("The file is missing") : drawer.item.error ?? ""
                    color: Kirigami.Theme.negativeTextColor
                }

                QQC2.Label {
                    Kirigami.FormData.label: App.tr("Package:")
                    visible: !!drawer.item.package
                    text: drawer.item.package ? [drawer.item.package.id, drawer.item.package.version, drawer.item.package.license].join(" · ") : ""
                }

                QQC2.Label {
                    Kirigami.FormData.label: App.tr("Installed from:")
                    visible: !!drawer.details.installed
                    Layout.maximumWidth: Kirigami.Units.gridUnit * 14
                    text: drawer.details.installed ? [drawer.repo || drawer.details.installed.repo_id.slice(0, 8), drawer.details.installed.signer ? App.tr("signed by %1").arg(drawer.details.installed.signer) : ""].filter(t => t).join("\n") : ""
                    wrapMode: Text.WrapAnywhere
                }

                QQC2.Label {
                    Kirigami.FormData.label: App.tr("Revoked:")
                    visible: !!drawer.details.revoked
                    text: Labels.value(drawer.details.revoked)
                    color: Kirigami.Theme.negativeTextColor
                }

                QQC2.Label {
                    Kirigami.FormData.label: App.tr("Credit:")
                    visible: !!drawer.credit
                    text: drawer.credit ? [drawer.credit.title, drawer.credit.attribution, drawer.credit.license, drawer.credit.link].filter(t => t).join("\n") : ""
                    wrapMode: Text.Wrap
                }
            }

            ItemExtras {
                Layout.fillWidth: true
                host: drawer
            }

            RowLayout {
                QQC2.Button {
                    text: App.tr("Edit…")
                    icon.name: "document-edit"
                    visible: drawer.item.kind === "image" && !drawer.item.missing
                    onClicked: {
                        drawer.close();
                        App.edit(drawer.item);
                    }
                }

                QQC2.Button {
                    text: App.tr("Apply to…")
                    icon.name: "preferences-desktop-wallpaper"
                    onClicked: drawer.apply(drawer.uid)
                }

                QQC2.Button {
                    text: App.tr("Remove…")
                    icon.name: "edit-delete"
                    enabled: drawer.removable
                    onClicked: drawer.remove(drawer.uid)
                }
            }
        }
    }
}
