import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: swing

    required property var motion
    readonly property var presets: motion.info?.presets ?? []
    readonly property var bodies: motion.info?.physics ?? []
    readonly property var words: ({
            "hair": App.tr("Hair"),
            "tail": App.tr("Tail"),
            "cloth": App.tr("Cloth"),
            "antenna": App.tr("Antenna"),
            "chain": App.tr("Chain"),
            "jelly": App.tr("Jelly")
        })

    visible: motion.bones.length > 0

    QQC2.Label {
        text: App.tr("Swinging bones")
        font.bold: true
    }

    SceneBonePick {
        Layout.fillWidth: true
        names: swing.motion.bones
        chosen: swing.motion.bone !== "" ? [swing.motion.bone] : []
        single: true
        onPicked: list => {
            swing.motion.bone = list[0] ?? "";
            swing.motion.page.mesh.bone = swing.motion.bone;
        }
    }

    Flow {
        Layout.fillWidth: true
        enabled: swing.motion.bone !== ""

        Repeater {
            model: swing.presets

            delegate: QQC2.Button {
                required property string modelData

                text: swing.words[modelData] ?? modelData
                onClicked: swing.motion.animate("preset", {
                    "bone": swing.motion.bone,
                    "preset": modelData
                })
            }
        }
    }

    Repeater {
        model: swing.bodies

        delegate: QQC2.Label {
            required property var modelData

            text: App.tr("%1 swings as %2, stiffness %3, wind %4", modelData.bone, SceneWords.label(modelData.mode ?? "spring"), modelData.stiffness ?? 0.5, modelData.wind ?? 0)
            font: Kirigami.Theme.smallFont
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: swing.bodies.length > 0
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Swinging stops at the lowest quality level, where the bones keep their animation.")
    }
}
