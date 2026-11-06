import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.PromptDialog {
    id: sheet

    required property var page

    title: App.tr("Import a sprite sheet or a data map")
    standardButtons: Kirigami.Dialog.Cancel
    customFooterActions: [
        Kirigami.Action {
            text: App.tr("Choose the picture…")
            icon.name: "document-open"
            onTriggered: picture.open()
        }
    ]

    Kirigami.FormLayout {
        QQC2.CheckBox {
            id: cut

            Kirigami.FormData.label: App.tr("Sprite sheet:")
            text: App.tr("Cut into equal cells")
        }

        QQC2.SpinBox {
            id: rows

            Kirigami.FormData.label: App.tr("Rows:")
            enabled: cut.checked
            from: 1
            to: 256
            value: 4
        }

        QQC2.SpinBox {
            id: columns

            Kirigami.FormData.label: App.tr("Columns:")
            enabled: cut.checked
            from: 1
            to: 256
            value: 4
        }

        QQC2.CheckBox {
            id: linear

            Kirigami.FormData.label: App.tr("Data:")
            text: App.tr("Read it as data, such as a normal map or a mask, not as colors")
        }
    }

    FileDialog {
        id: picture

        nameFilters: [App.tr("Pictures") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp *.ktx2 *.hdr *.exr)"]
        onAccepted: {
            const args = {
                "file": App.path(selectedFile),
                "linear": linear.checked
            };
            if (cut.checked)
                args.atlas = [rows.value, columns.value];
            sheet.page.run("import", args);
            sheet.close();
        }
    }
}
