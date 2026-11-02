import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: cost

    property var info: null

    function memory(bytes) {
        return App.tr("%1 MiB", (bytes / 1048576).toFixed(1));
    }

    visible: (info?.models ?? []).length > 0

    Kirigami.Heading {
        level: 5
        text: App.tr("3D models")
    }

    Repeater {
        model: cost.info?.levels ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            font.bold: modelData.quality === SceneLook.quality
            text: App.tr("%1: %2 triangles in %3 3D groups, %4 units of work, %5", modelData.quality.toUpperCase(), modelData.triangles, cost.info.groups, modelData.units, modelData.specular ? App.tr("with reflections") : App.tr("no reflections of pictures"))
        }
    }

    Repeater {
        model: cost.info?.tiers ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: modelData.fits ? Kirigami.Theme.textColor : Kirigami.Theme.negativeTextColor
            text: App.tr("%1 at %2: %3, %4 of textures at most %5 px", modelData.tier.toUpperCase(), modelData.quality.toUpperCase(), modelData.fits ? App.tr("fits") : App.tr("too heavy"), cost.memory(modelData.memory), modelData.texture) + (modelData.reasons.length > 0 ? " (" + modelData.reasons.join("; ") + ")" : "")
        }
    }
}
