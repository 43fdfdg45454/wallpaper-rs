import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: board

    required property var status
    property var strip: null
    property int chosen: 0
    readonly property var places: status.places ?? ({})
    readonly property var targets: {
        const found = [];
        for (const d of status.devices ?? []) {
            found.push({
                    "key": d.key ?? d.name,
                    "text": d.key ?? d.name,
                    "zone": -1
                });
            (d.zones ?? []).forEach((z, i) => found.push({
                        "key": (d.key ?? d.name) + "#" + i,
                        "text": App.tr("%1, zone %2", d.key ?? d.name, z.name),
                        "zone": i
                    }));
        }
        return found;
    }
    readonly property var target: targets[Math.min(chosen, targets.length - 1)] ?? null
    readonly property string value: target ? String(places[target.key] ?? "") : ""
    readonly property var rect: parse(value) ?? ({
            "x": 0,
            "y": 0,
            "w": 1,
            "h": 1,
            "flow": "right"
        })
    readonly property var flows: [
        {
            "value": "right",
            "text": App.tr("Lights run left to right")
        },
        {
            "value": "left",
            "text": App.tr("Lights run right to left")
        },
        {
            "value": "down",
            "text": App.tr("Lights run top to bottom")
        },
        {
            "value": "up",
            "text": App.tr("Lights run bottom to top")
        }
    ]

    signal saved
    signal failed(var error)

    function parse(text) {
        const parts = text.split(" ");
        if (parts.length < 4)
            return null;
        return {
            "x": Number(parts[0]),
            "y": Number(parts[1]),
            "w": Number(parts[2]),
            "h": Number(parts[3]),
            "flow": parts[4] ?? "right"
        };
    }

    function save(text) {
        App.act("SetOption", ["rgb.place." + target.key, text], "Daemon1", () => board && board.saved(), error => board && board.failed(error));
    }

    function put(x, y, w, h, flow) {
        const clamp = v => Math.round(Math.min(1, Math.max(0, v)) * 1000) / 1000;
        const width = clamp(Math.max(0.02, w)), height = clamp(Math.max(0.02, h));
        save([clamp(Math.min(x, 1 - width)), clamp(Math.min(y, 1 - height)), width, height, flow].join(" "));
    }

    Timer {
        interval: 500
        repeat: true
        running: board.visible && !!board.status.source
        onTriggered: App.query("GetStrip", [board.status.source], "Rgb1", reply => board && (board.strip = reply), () => board && (board.strip = null))
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: App.tr("Every device takes the whole picture until you place it. Choose a device or one of its zones, drag its frame over the picture and pull its corner to size it.")
    }

    QQC2.ComboBox {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 20
        Accessible.name: App.tr("Device or zone to place")
        model: board.targets
        textRole: "text"
        onActivated: board.chosen = currentIndex
    }

    RgbPicture {
        board: board
    }

    RowLayout {
        QQC2.ComboBox {
            Accessible.name: App.tr("Direction of the lights")
            model: board.flows
            textRole: "text"
            valueRole: "value"
            enabled: board.value !== "off"
            currentIndex: Math.max(0, board.flows.findIndex(f => f.value === board.rect.flow))
            onActivated: board.put(board.rect.x, board.rect.y, board.rect.w, board.rect.h, currentValue)
        }

        QQC2.Button {
            text: App.tr("Whole picture")
            enabled: board.value !== ""
            onClicked: board.save("none")
        }

        QQC2.Button {
            text: board.target?.zone >= 0 ? App.tr("Keep the zone dark") : App.tr("Leave the device alone")
            enabled: board.value !== "off"
            onClicked: board.save("off")
        }
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        color: Kirigami.Theme.disabledTextColor
        text: board.value === "off" ? (board.target?.zone >= 0 ? App.tr("This zone stays dark while the rest of the device follows the wallpaper.") : App.tr("This device keeps its own mode and gets no colors.")) : board.value === "" ? App.tr("Takes the whole picture.") : App.tr("Takes the framed part of the picture.")
    }
}
