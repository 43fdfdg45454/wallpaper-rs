import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: sections

    required property var page
    readonly property var pages: (page.view.files ?? []).filter(file => file.kind === "page").map(file => file.path)

    spacing: Kirigami.Units.largeSpacing

    Kirigami.FormLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Kind:")
            text: Labels.value(sections.page.kind)
        }

        QQC2.ComboBox {
            Kirigami.FormData.label: App.tr("Entry page:")
            visible: sections.page.kind === "web"
            model: sections.pages
            currentIndex: sections.pages.indexOf(sections.page.manifest.entry ?? "")
            onActivated: index => sections.page.change("/manifest/entry", sections.pages[index])
        }

        QQC2.Button {
            Kirigami.FormData.label: App.tr("Package:")
            text: App.tr("Title, license, origin and more…")
            icon.name: "documentinfo"
            onClicked: sections.page.editor.select("/manifest")
        }
    }
}
