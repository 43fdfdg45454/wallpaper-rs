import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

RowLayout {
    id: row

    required property var part
    property bool chosen
    property string picture

    signal toggled(bool on)
    signal renamed(string text)
    signal pick

    QQC2.CheckBox {
        Layout.fillWidth: true
        text: row.part.label + " · " + SceneWords.kind(row.part.kind)
        checked: row.chosen
        onToggled: row.toggled(checked)
    }

    QQC2.TextField {
        visible: row.chosen
        placeholderText: App.tr("Title of the part")
        onEditingFinished: row.renamed(text)
    }

    Image {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 3
        Layout.preferredHeight: Kirigami.Units.gridUnit * 2
        fillMode: Image.PreserveAspectFit
        visible: row.chosen && !!row.picture
        source: row.picture ? App.url(row.picture) : ""
    }

    QQC2.ToolButton {
        visible: row.chosen
        icon.name: "insert-image"
        text: App.tr("Thumbnail…")
        display: QQC2.AbstractButton.IconOnly
        onClicked: row.pick()
    }
}
