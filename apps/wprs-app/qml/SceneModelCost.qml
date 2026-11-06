import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: cost

    property var info: null

    function more(row) {
        const out = [];
        if ((row.shadow_texels ?? 0) > 0)
            out.push(App.tr("shadow maps of %1 megapixels", (row.shadow_texels / 1e6).toFixed(1)));
        if ((row.fur_shells ?? 0) > 0)
            out.push(App.tr("%1 fur shells over %2 triangles", row.fur_shells, row.fur_triangles));
        if ((row.skinned ?? 0) > 0)
            out.push(row.physics ? App.tr("%1 skinned vertices with swinging bones", row.skinned) : App.tr("%1 skinned vertices, bones keep their animation", row.skinned));
        return out.length > 0 ? "; " + out.join(", ") : "";
    }

    function memory(bytes) {
        return App.tr("%1 MiB", (bytes / 1048576).toFixed(1));
    }

    visible: (info?.models ?? []).length > 0

    Kirigami.Heading {
        level: 5
        text: App.tr("3D models")
    }

    SceneRepeater {
        id: listed2

        rows: cost.info?.levels ?? []

        delegate: QQC2.Label {
            required property int index
            readonly property var modelData: listed2.rows[index]

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            font.bold: modelData.quality === SceneLook.quality
            text: App.tr("%1: %2 triangles in %3 3D groups, %4 units of work, %5", modelData.quality.toUpperCase(), modelData.triangles, cost.info.groups, modelData.units, modelData.specular ? App.tr("with reflections") : App.tr("no reflections of pictures")) + cost.more(modelData)
        }
    }

    SceneRepeater {
        id: listed

        rows: cost.info?.tiers ?? []

        delegate: QQC2.Label {
            required property int index
            readonly property var modelData: listed.rows[index]

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: modelData.fits ? Kirigami.Theme.textColor : Kirigami.Theme.negativeTextColor
            text: App.tr("%1 at %2: %3, %4 of textures at most %5 px", modelData.tier.toUpperCase(), modelData.quality.toUpperCase(), modelData.fits ? App.tr("fits") : modelData.poster ? App.tr("shows its poster") : App.tr("too heavy"), cost.memory(modelData.memory), modelData.texture) + (modelData.reasons.length > 0 ? " (" + modelData.reasons.join("; ") + ")" : "") + ", " + App.tr("%1 lights with shadows", modelData.shadows ?? 0)
        }
    }
}
