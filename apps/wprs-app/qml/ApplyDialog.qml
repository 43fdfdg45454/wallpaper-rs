import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    property string uid
    property var presets: []
    property bool flashing
    property bool known
    readonly property var fits: App.schema.fits ?? ["fill", "fit", "stretch", "center", "tile"]

    function show(uid) {
        dialog.uid = uid;
        dialog.presets = [];
        dialog.flashing = false;
        dialog.known = false;
        App.query("GetItem", [uid], "Library1", reply => {
            if (dialog.uid !== uid)
                return;
            const pkg = reply.item.package;
            dialog.flashing = !!pkg && pkg.photosensitive === true && !(pkg.confirmed ?? []).includes("photosensitive");
            dialog.known = true;
        });
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
    onOpened: standardButton(Kirigami.Dialog.Ok).enabled = Qt.binding(() => dialog.known)
    onAccepted: {
        const args = [uid, output.currentValue, fit.currentValue, preset.currentValue];
        if (flashing)
            App.act("Confirm", [[uid], "photosensitive"], "Library1", () => App.act("Apply", args, "Library1"));
        else
            App.act("Apply", args, "Library1");
    }

    Kirigami.FormLayout {
        Kirigami.InlineMessage {
            Layout.fillWidth: true
            visible: dialog.flashing
            type: Kirigami.MessageType.Warning
            text: App.tr("This wallpaper flashes or shows fast patterns that can trigger seizures in people with photosensitive epilepsy. Apply it only if that is safe for whoever sees this screen; you will not be asked again for this package.")
        }

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
