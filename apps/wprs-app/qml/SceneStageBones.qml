import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: bones

    required property var stage
    required property var model
    readonly property var list: model.bones ?? []
    readonly property var path: model.path ?? []
    readonly property bool shown: stage.selected === model.pointer

    visible: list.length > 0 && shown

    SceneLightLines {
        anchors.fill: parent
        ratio: bones.stage.ratio
        tint: Kirigami.Theme.highlightColor
        segments: bones.list.filter(b => b.from).map(b => [b.from, b.at])
    }

    SceneLightLines {
        anchors.fill: parent
        ratio: bones.stage.ratio
        dashed: true
        tint: Kirigami.Theme.positiveTextColor
        segments: bones.path.slice(1).map((p, i) => [bones.path[i], p])
    }

    SceneBoneSwing3d {
        anchors.fill: parent
        bones: bones
    }

    SceneBonePose3d {
        anchors.fill: parent
        bones: bones
    }

    SceneRepeater {
        id: listed

        rows: bones.list

        delegate: Rectangle {
            required property int index
            readonly property var modelData: listed.rows[index]
            readonly property bool chosen: bones.stage.page.mesh.bone === modelData.name

            x: modelData.at[0] * bones.stage.ratio - width / 2
            y: modelData.at[1] * bones.stage.ratio - height / 2
            width: Kirigami.Units.gridUnit * (chosen ? 0.8 : 0.55)
            height: width
            radius: width / 2
            color: chosen ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.highlightColor
            border.color: Kirigami.Theme.backgroundColor
            QQC2.ToolTip.visible: hover.hovered
            QQC2.ToolTip.text: modelData.name

            HoverHandler {
                id: hover
            }

            TapHandler {
                gesturePolicy: TapHandler.ReleaseWithinBounds
                onTapped: {
                    bones.stage.page.mesh.layer = bones.model.id;
                    bones.stage.page.mesh.bone = parent.modelData.name;
                }
            }
        }
    }
}
