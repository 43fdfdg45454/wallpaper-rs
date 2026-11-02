import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: card

    property var info: null

    function size(bytes) {
        return bytes > 1048576 ? App.tr("%1 MiB", (bytes / 1048576).toFixed(1)) : App.tr("%1 KiB", Math.ceil(bytes / 1024));
    }

    visible: info !== null && info.error === undefined

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: App.tr("%1 triangles, %2 materials, %3 textures, %4 bones, %5 clips", card.info?.triangles ?? 0, (card.info?.materials ?? []).length, (card.info?.textures ?? []).length, card.info?.bones ?? 0, (card.info?.clips ?? []).length)
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        visible: (card.info?.clips ?? []).length > 0
        text: (card.info?.clips ?? []).map(c => App.tr("%1 (%2 s)", c.name, c.length)).join(" · ")
    }

    Repeater {
        model: card.info?.tiers ?? []

        delegate: QQC2.Label {
            required property var modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: modelData.fits ? Kirigami.Theme.textColor : Kirigami.Theme.negativeTextColor
            text: App.tr("%1: %2, textures at most %3 px, %4 of texture memory", modelData.tier.toUpperCase(), modelData.fits ? App.tr("fits") : App.tr("too heavy"), modelData.texture, card.size(modelData.memory)) + (modelData.reasons.length > 0 ? " (" + modelData.reasons.join("; ") + ")" : "")
        }
    }

    Repeater {
        model: card.info?.notes ?? []

        delegate: QQC2.Label {
            required property string modelData

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: Kirigami.Theme.neutralTextColor
            text: modelData
        }
    }
}
