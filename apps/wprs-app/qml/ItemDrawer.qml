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
    readonly property var pkg: item.package ?? null
    readonly property bool mature: !!pkg && pkg.content === "mature" && !(pkg.confirmed ?? []).includes("mature") && !App.unveiled
    readonly property bool flashing: !!pkg && pkg.photosensitive === true && !(pkg.confirmed ?? []).includes("photosensitive")
    readonly property string animated: mature || flashing ? "" : App.url(details.about?.animated ?? "")

    signal apply(string uid)
    signal remove(string uid)

    function show(uid) {
        drawer.uid = uid;
        details = {};
        preview = "";
        repo = "";
        load(true);
        open();
    }

    function load(first) {
        const uid = drawer.uid;
        App.query("GetItem", [uid], "Library1", reply => {
            if (!drawer)
                return;
            if (drawer.uid !== uid)
                return;
            drawer.details = reply;
            if (first)
                App.query("Thumbnail", [uid, 1024], "Library1", path => drawer && drawer.uid === uid && (drawer.preview = path[0] ?? ""), () => {});
            if (reply.installed)
                App.query("List", [], "Repos1", list => {
                    if (!drawer)
                        return;
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
                visible: !moving.visible
                Layout.fillWidth: true
                Layout.preferredHeight: width * 9 / 16
                source: App.url(drawer.preview)
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: false
                sourceSize: drawer.mature ? Qt.size(16, 16) : undefined
            }

            AnimatedImage {
                id: moving

                Layout.fillWidth: true
                Layout.preferredHeight: width * 9 / 16
                visible: drawer.animated !== "" && status === AnimatedImage.Ready
                source: drawer.animated
                playing: visible && drawer.opened && !(App.state.on_battery ?? false)
                fillMode: Image.PreserveAspectFit
                cache: false
            }

            QQC2.Button {
                visible: drawer.mature
                text: App.tr("Show mature content")
                icon.name: "view-visible"
                onClicked: App.act("Confirm", [[drawer.uid], "mature"], "Library1")
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

            ItemFacts {
                Layout.fillWidth: true
                host: drawer
            }

            ItemKin {
                Layout.fillWidth: true
                host: drawer
            }

            PackageAbout {
                Layout.fillWidth: true
                visible: !!drawer.details.about
                info: drawer.details.about ?? {}
                uid: drawer.uid
                pkg: drawer.pkg
            }

            ItemExtras {
                Layout.fillWidth: true
                host: drawer
            }

            QQC2.Switch {
                Layout.fillWidth: true
                visible: drawer.pkg?.kind === "scene" && drawer.pkg?.photosensitive !== true
                text: App.tr("Keep moving when the desktop asks for reduced motion")
                checked: (drawer.pkg?.confirmed ?? []).includes("motion")
                onToggled: {
                    App.act(checked ? "Confirm" : "Unconfirm", [[drawer.uid], "motion"], "Library1");
                    checked = Qt.binding(() => (drawer.pkg?.confirmed ?? []).includes("motion"));
                }
            }

            WebPrivacy {
                Layout.fillWidth: true
                uid: drawer.uid
                pkg: drawer.pkg
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
