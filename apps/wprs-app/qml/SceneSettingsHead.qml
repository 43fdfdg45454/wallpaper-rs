import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: head

    required property var page

    QQC2.ItemDelegate {
        Layout.fillWidth: true
        highlighted: head.page.editor.selected === "/manifest"
        icon.name: "documentinfo"
        text: App.tr("Package details")
        onClicked: head.page.editor.select("/manifest")

        QQC2.ToolTip.text: (head.page.view.title ?? "") + " · " + (head.page.view.version ?? "")
        QQC2.ToolTip.visible: hovered
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: head.page.view.v2 === false
        text: App.tr("This package uses the first version of the manifest. Upgrade it to give it a rating, credits and texts in several languages; what it cannot tell, such as the rating, the flashes and the origin, stays among the problems until you set it.")
        actions: [
            Kirigami.Action {
                text: App.tr("Upgrade")
                icon.name: "go-up"
                onTriggered: head.page.run("upgrade")
            }
        ]
    }

    Kirigami.Heading {
        Layout.fillWidth: true
        level: 4
        text: App.tr("User properties")
    }
}
