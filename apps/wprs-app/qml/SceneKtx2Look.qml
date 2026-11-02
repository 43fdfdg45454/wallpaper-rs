import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: look

    property string source: ""
    property string label: ""
    property string detail: ""

    Image {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
        Layout.preferredHeight: Kirigami.Units.gridUnit * 10
        source: look.source ? App.url(look.source) : ""
        cache: false
        smooth: false
        fillMode: Image.PreserveAspectFit
    }

    QQC2.Label {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 10
        wrapMode: Text.Wrap
        text: look.label
    }

    QQC2.Label {
        opacity: 0.7
        text: look.detail
    }
}
