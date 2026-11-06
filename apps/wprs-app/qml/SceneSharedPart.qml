import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: card

    required property var part
    property bool chosen

    signal toggled(bool on)

    Layout.preferredWidth: Kirigami.Units.gridUnit * 13
    Layout.alignment: Qt.AlignTop

    Image {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 13
        Layout.preferredHeight: Kirigami.Units.gridUnit * 7
        fillMode: Image.PreserveAspectFit
        asynchronous: true
        visible: !!card.part.thumbnail
        source: card.part.thumbnail ? App.url(card.part.thumbnail) : ""
    }

    Kirigami.Icon {
        Layout.preferredWidth: Kirigami.Units.iconSizes.huge
        Layout.preferredHeight: Kirigami.Units.iconSizes.huge
        visible: !card.part.thumbnail
        source: ({
                "layer": "layer-visible-on",
                "particles": "snowflake",
                "script": "text-x-script",
                "effect": "special-effects-symbolic"
            })[card.part.kind] ?? "package-x-generic"
    }

    QQC2.CheckBox {
        Layout.fillWidth: true
        text: card.part.title + " · " + SceneWords.kind(card.part.kind)
        checked: card.chosen
        onToggled: card.toggled(checked)
    }

    SceneRepeater {
        id: listed

        rows: card.part.files ?? []

        delegate: QQC2.Label {
            required property int index
            readonly property var modelData: listed.rows[index]

            Layout.fillWidth: true
            wrapMode: Text.Wrap
            font: Kirigami.Theme.smallFont
            text: App.tr("%1 · %2 · %3", modelData.path, modelData.license, modelData.attribution)
        }
    }
}
