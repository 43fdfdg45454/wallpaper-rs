import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: strands

    required property var tools
    readonly property var info: tools.info?.strands ?? ({})
    readonly property var textures: ["none"].concat(info.textures ?? [])

    function set(strand, settings, gesture) {
        tools.edit("strand", {
                "strand": strand,
                "settings": settings
            }, gesture);
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Ropes and trails")
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Texture")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: strands.textures.map(t => t === "none" ? App.tr("No texture") : t)
            currentIndex: Math.max(0, strands.textures.indexOf(strands.info.texture ?? "none"))
            onActivated: index => strands.tools.edit("texture", index > 0 ? {
                        "texture": strands.textures[index]
                    } : {})
        }
    }

    QQC2.CheckBox {
        text: App.tr("Join particles with a rope")
        checked: !!strands.info.rope
        onToggled: checked ? strands.set("rope", {
                "width": 4
            }) : strands.tools.edit("strand", {
                "strand": "rope",
                "off": true
            })
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: !!strands.info.rope && (strands.info.uneven ?? []).length > 0
        type: Kirigami.MessageType.Information
        text: App.tr("Emitters that start more than one particle a step bunch the rope; one a step spaces it evenly")
        actions: [
            Kirigami.Action {
                text: App.tr("One particle a step")
                onTriggered: strands.tools.edit("pace", {
                        "max_per_step": 1
                    })
            }
        ]
    }

    SceneSwarmStrand {
        Layout.fillWidth: true
        visible: !!strands.info.rope
        strands: strands
        strand: "rope"
        value: strands.info.rope ?? ({})
        fields: [["width", App.tr("Width"), 0, 64, 4, 0.5], ["subdivide", App.tr("Points between particles"), 0, 7, 2, 1], ["uv_scale", App.tr("Texture repeats"), 0, 10, 1, 0.1], ["uv_scroll", App.tr("Texture scroll"), -5, 5, 0, 0.1]]
        toggles: [["smooth", App.tr("Smooth curve"), true]]
    }

    QQC2.CheckBox {
        text: App.tr("Leave a trail")
        checked: !!strands.info.trail
        onToggled: checked ? strands.set("trail", {}) : strands.tools.edit("strand", {
                "strand": "trail",
                "off": true
            })
    }

    SceneSwarmStrand {
        Layout.fillWidth: true
        visible: !!strands.info.trail
        strands: strands
        strand: "trail"
        value: strands.info.trail ?? ({})
        fields: [["length", App.tr("Steps"), 1, 64, 8, 1], ["width", App.tr("Width"), 0, 1, 0.5, 0.05], ["segments", App.tr("Kept points"), 1, 64, 8, 1], ["subdivide", App.tr("Points between kept points"), 0, 7, 0, 1], ["uv_scale", App.tr("Texture repeats"), 0, 10, 1, 0.1], ["uv_scroll", App.tr("Texture scroll"), -5, 5, 0, 0.1]]
        toggles: [["textured", App.tr("Show the texture along the trail"), false]]
    }
}
