import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents3

ColumnLayout {
    id: row

    required property var daemon
    required property int index
    required property string name
    required property var info
    readonly property string item: info.item ?? ""
    readonly property var playing: info.playlist ?? null
    readonly property string title: details.title || info.credit?.title || String(info.image ?? "").split("/").pop() || (item ? "" : i18n("Background color"))
    property string thumb
    property LeftOut reasons: LeftOut {}
    property var details: ({})
    property string animated
    readonly property var pkg: details.package ?? {}
    readonly property var confirmed: pkg.confirmed ?? []
    readonly property bool mature: pkg.content === "mature" && !confirmed.includes("mature")
    readonly property bool allowed: confirmed.includes("photosensitive")

    function load() {
        const item = row.item;
        if (!item)
            return;
        daemon.query("GetItem", [item], "Library1", reply => {
            if (row.item !== item)
                return;
            row.details = reply.item ?? {};
            row.animated = reply.about?.animated ?? "";
            daemon.query("Thumbnail", [item, 256], "Library1", answer => {
                if (row.item === item)
                    row.thumb = answer[0] ?? "";
            });
        });
    }

    spacing: Kirigami.Units.smallSpacing
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

    Kirigami.Separator {
        Layout.fillWidth: true
        Layout.bottomMargin: Kirigami.Units.smallSpacing
        visible: row.index > 0
    }

    RowLayout {
        spacing: Kirigami.Units.largeSpacing

        Thumbnail {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 4
            Layout.preferredHeight: Kirigami.Units.gridUnit * 2.5
            color: row.info.background || Kirigami.Theme.alternateBackgroundColor
            daemon: row.daemon
            item: row.item
            source: row.daemon.url(row.thumb)
            mature: row.mature
            animated: row.pkg.photosensitive === true && !row.allowed ? "" : row.daemon.url(row.animated)
            moving: !(row.daemon.state?.on_battery ?? false)
            scene: row.info.kind === "scene"
        }

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 0

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                text: row.title
                textFormat: Text.PlainText
                elide: Text.ElideRight
            }

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                text: row.info.error || [row.name].concat(row.playing ? [i18n("%1 of %2", row.playing.position + 1, row.playing.count), row.playing.name] : []).join(" · ")
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font: Kirigami.Theme.smallFont
                color: row.info.error ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.disabledTextColor
            }

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                visible: (row.info.left_out ?? []).length > 0
                text: (row.info.left_out ?? []).map(l => reasons.text(l)).join("\n")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
            }

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                visible: row.info.particles === "cpu"
                text: i18n("Its particles run on the processor, so fewer of them are shown")
                wrapMode: Text.Wrap
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
            }

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                visible: !!row.info.quality
                text: row.info.quality ? row.reasons.level(row.info, row.daemon.current) : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font: Kirigami.Theme.smallFont
                color: Kirigami.Theme.disabledTextColor
            }

            PlasmaComponents3.Switch {
                Layout.fillWidth: true
                visible: row.daemon.inputs.shown?.[row.name]?.clicks === true
                text: i18n("Let clicks and taps on this screen reach its wallpaper")
                checked: (row.daemon.inputs.clicks ?? []).includes(row.name)
                onToggled: {
                    row.daemon.act("SetClicks", [row.name, checked], "Inputs1");
                    checked = Qt.binding(() => (row.daemon.inputs.clicks ?? []).includes(row.name));
                }
            }

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                visible: row.pkg.photosensitive === true
                text: row.allowed ? i18n("Flashes or shows fast patterns; you allowed it") : i18n("Flashes or shows fast patterns that can trigger seizures")
                wrapMode: Text.Wrap
                font: Kirigami.Theme.smallFont
                color: row.allowed ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.negativeTextColor
            }
        }
    }

    ScreenButtons {
        Layout.fillWidth: true
        host: row
    }
}
