import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

RowLayout {
    id: row

    required property var daemon
    required property string name
    required property var info
    required property var playlists
    readonly property string item: info.item ?? ""
    readonly property var playing: info.playlist ?? null
    readonly property var credit: info.credit ?? null
    readonly property var fits: daemon.schema.fits ?? ["fill", "fit", "stretch", "center", "tile"]
    readonly property bool scene: info.kind === "scene"
    readonly property bool tunable: scene || (info.kind === "video" && pkg.kind === "video")
    readonly property string sound: info.sound ?? ""
    readonly property bool muted: daemon.state?.muted === true
    property string thumb
    property var details: ({})
    property string animated
    readonly property var pkg: details.package ?? {}
    readonly property var confirmed: pkg.confirmed ?? []
    readonly property bool favorite: details.favorite === true
    readonly property bool mature: pkg.content === "mature" && !confirmed.includes("mature")
    readonly property bool allowed: confirmed.includes("photosensitive")

    LeftOut {
        id: reasons
        daemon: row.daemon
    }
    function load() {
        const item = row.item;
        if (!item)
            return;
        daemon.query("GetItem", [item], "Library1", reply => {
            if (!row || row.item !== item)
                return;
            row.details = reply.item ?? {};
            row.animated = reply.about?.animated ?? "";
            daemon.query("Thumbnail", [item, 256], "Library1", answer => {
                if (!row)
                    return;
                if (row.item === item)
                    row.thumb = answer[0] ?? "";
            });
        });
    }

    onItemChanged: {
        thumb = "";
        animated = "";
        details = {};
        Qt.callLater(load);
    }
    Component.onCompleted: Qt.callLater(load)

    Connections {
        target: row.daemon

        function onUpdated(topic, ids) {
            if (topic !== "library")
                return;
            if (ids.includes(row.item))
                row.load();
            else if (ids.length === 0)
                settle.restart();
        }
    }

    Timer {
        id: settle

        interval: 1000
        onTriggered: row.load()
    }

    Thumbnail {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 6
        Layout.preferredHeight: Kirigami.Units.gridUnit * 4
        visible: row.item !== ""
        daemon: row.daemon
        item: row.item
        source: row.daemon.url(row.thumb)
        mature: row.mature
        animated: row.pkg.photosensitive === true && !row.allowed ? "" : row.daemon.url(row.animated)
        moving: !(row.daemon.state?.on_battery ?? false)
        scene: row.scene
    }

    ColumnLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Layout.fillWidth: true
            elide: Text.ElideMiddle
            font.bold: true
            text: row.name ? row.name + (row.info.width ? " · " + row.info.width + "×" + row.info.height : "") : row.daemon.tr("Every screen")
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.name !== ""
            elide: Text.ElideMiddle
            text: row.info.error || (row.playing ? row.daemon.tr("Playlist %1 (%2 of %3)", row.playing.name, row.playing.position + 1, row.playing.count) : (row.info.image ?? "").split("/").pop()) || row.daemon.tr("Background color")
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.pkg.photosensitive === true
            wrapMode: Text.Wrap
            text: row.allowed ? row.daemon.tr("Flashes or shows fast patterns; you allowed it") : row.daemon.tr("Flashes or shows fast patterns that can trigger seizures")
            color: row.allowed ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.negativeTextColor
            font: Kirigami.Theme.smallFont
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.mature
            wrapMode: Text.Wrap
            text: row.daemon.tr("Mature content: click the picture to always show this package")
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.credit !== null
            elide: Text.ElideRight
            text: row.credit ? [row.credit.title, row.credit.attribution, row.credit.license].filter(t => t).join(" · ") : ""
            textFormat: Text.PlainText
            font: Kirigami.Theme.smallFont
        }

        RowLayout {
            Layout.fillWidth: true
            visible: (row.info.left_out ?? []).length > 0

            QQC2.Label {
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                text: (row.info.left_out ?? []).map(l => reasons.text(l)).join("\n")
                textFormat: Text.PlainText
                color: Kirigami.Theme.disabledTextColor
                font: Kirigami.Theme.smallFont
            }

            QQC2.ToolButton {
                text: row.daemon.tr("Try again")
                icon.name: "view-refresh"
                onClicked: row.daemon.act("RetryEffects", [])
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.info.particles === "cpu"
            wrapMode: Text.Wrap
            text: row.daemon.tr("Its particles run on the processor, so fewer of them are shown")
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: !!row.info.quality
            wrapMode: Text.Wrap
            text: row.info.quality ? reasons.level(row.info, row.daemon.state) : ""
            textFormat: Text.PlainText
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: (row.info.shown_fit ?? row.info.fit) !== row.info.fit
            wrapMode: Text.Wrap
            text: row.daemon.tr("This picture has its own placement: %1", row.daemon.value(row.info.shown_fit ?? ""))
            textFormat: Text.PlainText
            color: Kirigami.Theme.disabledTextColor
            font: Kirigami.Theme.smallFont
        }

        OutputControls {
            Layout.fillWidth: true
            host: row
        }
    }
}
