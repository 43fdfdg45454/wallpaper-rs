import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: reach

    required property var owner
    readonly property var page: owner.host.input.page
    readonly property var mesh: page.mesh
    readonly property string layerId: owner.host.layerId
    readonly property var layers: (page.view.layers ?? []).map(l => l.id).filter(id => id !== layerId)
    readonly property var choices: [null].concat(owner.targets, layers.map(id => "layer:" + id))

    function label(t) {
        if (t === null || Array.isArray(t))
            return App.tr("A point");
        if (t === "cursor")
            return App.tr("The pointer");
        return t.startsWith("layer:") ? App.tr("Layer %1", t.slice(6)) : t.slice(5);
    }

    function middle() {
        const size = owner.listed("/size", null) ?? page.view.canvas?.size ?? [2, 2];
        return [Math.round(size[0] / 2), Math.round(size[1] / 2)];
    }

    function run(index, args, gesture) {
        page.run("ik", Object.assign({
            "layer": layerId,
            "index": index
        }, args), gesture);
    }

    QQC2.Label {
        text: App.tr("Chains that reach")
        font.bold: true
    }

    Repeater {
        model: reach.owner.chains

        delegate: ColumnLayout {
            id: chain

            required property var modelData
            required property int index
            readonly property var target: modelData.target

            Layout.fillWidth: true

            RowLayout {
                QQC2.Label {
                    Layout.fillWidth: true
                    text: chain.modelData.chain.join(" → ")
                    elide: Text.ElideRight
                }

                QQC2.ToolButton {
                    icon.name: "object-flip-horizontal"
                    text: App.tr("Bend the other way")
                    display: QQC2.AbstractButton.IconOnly
                    checkable: true
                    checked: chain.modelData.bend === "counterclockwise"
                    QQC2.ToolTip.text: text
                    QQC2.ToolTip.visible: hovered
                    onToggled: reach.run(chain.index, {
                        "bend": checked ? "counterclockwise" : "clockwise"
                    })
                }

                QQC2.ToolButton {
                    icon.name: "edit-delete"
                    text: App.tr("Remove")
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: reach.run(chain.index, {
                        "remove": true
                    })
                }
            }

            QQC2.ComboBox {
                Layout.fillWidth: true
                model: reach.choices.map(t => App.tr("Reaches for %1", reach.label(t)))
                currentIndex: Array.isArray(chain.target) ? 0 : Math.max(0, reach.choices.indexOf(chain.target))
                onActivated: index => reach.run(chain.index, {
                        "target": index === 0 ? (Array.isArray(chain.target) ? chain.target : reach.middle()) : reach.choices[index]
                    })
            }

            RowLayout {
                QQC2.Label {
                    text: App.tr("Weight")
                }

                QQC2.Slider {
                    Layout.fillWidth: true
                    value: chain.modelData.weight ?? 1
                    onMoved: reach.run(chain.index, {
                        "weight": Math.round(value * 100) / 100
                    }, true)
                }

                QQC2.ComboBox {
                    id: limited

                    model: chain.modelData.chain
                }

                QQC2.Button {
                    readonly property bool has: (chain.modelData.limits ?? {})[limited.currentText] !== undefined

                    text: has ? App.tr("Free") : App.tr("Limit")
                    onClicked: reach.run(chain.index, has ? {
                        "unlimit": [limited.currentText]
                    } : {
                        "limits": reach.owner.pair(limited.currentText, [-45, 45])
                    })
                }
            }
        }
    }

    QQC2.Button {
        text: App.tr("Make a chain on the preview")
        icon.name: "draw-path"
        checkable: true
        enabled: reach.owner.names.length >= 2
        checked: reach.mesh.mode === "chain" && reach.mesh.rigging === reach.layerId
        onToggled: reach.mesh.use(reach.layerId, checked ? "chain" : "")
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: reach.mesh.mode === "chain" || reach.owner.chains.length > 0
        wrapMode: Text.Wrap
        text: reach.mesh.mode !== "chain" ? App.tr("Drag the ring of a chain that reaches for a point, and the dots of its limits, on the preview.") : reach.mesh.root === "" ? App.tr("Click the bone the chain starts from.") : App.tr("Now click the bone it ends at, below %1.", reach.mesh.root)
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: reach.owner.chains.some(c => c.target === "cursor")
        wrapMode: Text.Wrap
        text: App.tr("A chain that follows the pointer needs the pointer permission, and the scene tells the user that it follows the pointer.")
    }
}
