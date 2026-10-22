import QtQuick
import QtQuick.Dialogs
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    property string item
    property string shade
    readonly property var fits: [""].concat(App.schema.fits ?? ["fill", "fit", "stretch", "center", "tile"])

    title: item ? App.tr("New preset for this item") : App.tr("New preset")
    standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
    onAccepted: {
        const values = {};
        if (fit.currentValue)
            values.fit = fit.currentValue;
        if (shade)
            values.background = shade;
        App.act("SavePreset", [item, name.text, values], "Profiles1");
    }

    Kirigami.FormLayout {
        QQC2.TextField {
            id: name

            Kirigami.FormData.label: App.tr("Name:")
        }

        QQC2.ComboBox {
            id: fit

            Kirigami.FormData.label: Labels.field("fit") + ":"
            model: dialog.fits
            displayText: currentValue ? Labels.value(currentValue) : App.tr("Keep")
            delegate: QQC2.ItemDelegate {
                required property string modelData

                text: modelData ? Labels.value(modelData) : App.tr("Keep")
            }
        }

        QQC2.Button {
            Kirigami.FormData.label: Labels.field("background") + ":"
            text: dialog.shade || App.tr("Keep")
            icon.name: "color-picker"
            onClicked: colors.open()
        }
    }

    ColorDialog {
        id: colors

        onAccepted: dialog.shade = selectedColor.toString()
    }
}
