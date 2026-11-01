import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: parts

    required property var host
    readonly property var page: host.page
    readonly property var mesh: page.mesh
    readonly property string layerId: host.layerId
    readonly property real tall: Kirigami.Units.gridUnit * 2.2
    property var sorted: []

    function load() {
        const own = JSON.parse(page.editor.value(host.base + "/parts") || "null") ?? {};
        const names = host.info.parts ?? [];
        const rank = n => typeof own[n]?.order === "number" ? own[n].order : 0;
        sorted = names.map((n, i) => ({
                    "name": n,
                    "file": i,
                    "visible": own[n]?.visible !== false,
                    "over": own[n]?.clip_to ?? null
                })).sort((a, b) => rank(a.name) - rank(b.name) || a.file - b.file);
    }

    function run(args) {
        page.run("parts", Object.assign({
            "layer": layerId
        }, args));
    }

    function drop(from, to) {
        const list = sorted.map(p => p.name);
        const moved = list.splice(from, 1);
        list.splice(Math.max(0, Math.min(list.length, to)), 0, moved[0]);
        const order = {};
        list.forEach((n, i) => order[n] = i);
        run({
            "order": order
        });
    }

    function pair(key, value) {
        const out = {};
        out[key] = value;
        return out;
    }

    function pick(name) {
        mesh.layer = layerId;
        mesh.part = mesh.part === name ? "" : name;
        if (mesh.rigging !== layerId)
            mesh.use(layerId, "parts");
    }

    visible: (host.info.parts ?? []).length > 1
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            parts.load();
        }

        target: parts.page.editor
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    QQC2.Label {
        text: App.tr("Parts, from back to front")
        font.bold: true
    }

    Item {
        id: list

        Layout.fillWidth: true
        Layout.preferredHeight: parts.sorted.length * parts.tall

        Repeater {
            model: parts.sorted

            delegate: RowLayout {
                id: row

                required property var modelData
                required property int index

                width: list.width
                height: parts.tall
                y: index * parts.tall
                z: grab.active ? 1 : 0

                Kirigami.Icon {
                    Layout.preferredWidth: Kirigami.Units.iconSizes.small
                    Layout.preferredHeight: Kirigami.Units.iconSizes.small
                    source: "handle-sort"

                    DragHandler {
                        id: grab

                        target: row
                        xAxis.enabled: false
                        onActiveChanged: {
                            if (active)
                                return;
                            const to = Math.round(row.y / parts.tall);
                            row.y = Qt.binding(() => row.index * parts.tall);
                            if (to !== row.index)
                                parts.drop(row.index, to);
                        }
                    }
                }

                QQC2.CheckBox {
                    checked: row.modelData.visible
                    QQC2.ToolTip.text: App.tr("Shown")
                    QQC2.ToolTip.visible: hovered
                    onToggled: parts.run(checked ? {
                        "show": [row.modelData.name]
                    } : {
                        "hide": [row.modelData.name]
                    })
                }

                QQC2.Button {
                    Layout.fillWidth: true
                    text: row.modelData.name
                    flat: true
                    checkable: true
                    checked: parts.mesh.part === row.modelData.name
                    onClicked: parts.pick(row.modelData.name)
                }

                QQC2.ComboBox {
                    readonly property var choices: [null].concat(parts.sorted.map(p => p.name).filter(n => n !== row.modelData.name))

                    model: choices.map(c => c === null ? App.tr("Over everything") : App.tr("Only over %1", c))
                    currentIndex: Math.max(0, choices.indexOf(row.modelData.over))
                    onActivated: index => parts.run(index === 0 ? {
                            "unclip": [row.modelData.name]
                        } : {
                            "clip": parts.pair(row.modelData.name, choices[index])
                        })
                }
            }
        }
    }
}
