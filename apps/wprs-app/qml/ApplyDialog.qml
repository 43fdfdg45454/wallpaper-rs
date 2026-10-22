import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    property string uid
    property var presets: []
    readonly property var fits: App.schema.fits ?? ["fill", "fit", "stretch", "center", "tile"]

    function show(uid) {
        dialog.uid = uid;
        dialog.presets = [];
        for (const item of [uid, ""])
            App.query("ListPresets", [item], "Profiles1", reply => dialog.presets = dialog.presets.concat(reply.presets));
        open();
    }

    function choices(first, values, text) {
        return [
            {
                "value": "",
                "text": first
            }
        ].concat(values.map(value => ({
                    "value": value.uid ?? value,
                    "text": text(value)
                })));
    }

    title: App.tr("Apply to…")
    standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
    onAccepted: App.act("Apply", [uid, output.currentValue, fit.currentValue, preset.currentValue], "Library1")

    Kirigami.FormLayout {
        QQC2.ComboBox {
            id: output

            Kirigami.FormData.label: App.tr("Screen:")
            model: dialog.choices(App.tr("All screens"), App.outputs, name => name)
            textRole: "text"
            valueRole: "value"
        }

        QQC2.ComboBox {
            id: fit

            Kirigami.FormData.label: App.tr("Placement:")
            model: dialog.choices(App.tr("Keep"), dialog.fits, value => Labels.value(value))
            textRole: "text"
            valueRole: "value"
        }

        QQC2.ComboBox {
            id: preset

            Kirigami.FormData.label: App.tr("Preset:")
            model: dialog.choices(App.tr("None"), dialog.presets, value => value.name)
            textRole: "text"
            valueRole: "value"
        }
    }
}
