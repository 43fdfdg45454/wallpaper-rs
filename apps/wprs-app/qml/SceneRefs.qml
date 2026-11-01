import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: refs

    required property var input
    readonly property var field: input.field
    readonly property string kind: field.widget.split(":")[0]
    readonly property var value: input.value
    readonly property var keys: Object.keys(value ?? {})

    function label(option) {
        if (Array.isArray(option))
            return App.tr("A point");
        if (option === "cursor")
            return App.tr("The pointer");
        const text = String(option);
        if (text.startsWith("bone:"))
            return App.tr("Bone %1", text.slice(5));
        return text.startsWith("layer:") ? App.tr("Layer %1", text.slice(6)) : text;
    }

    QQC2.ComboBox {
        readonly property var options: refs.input.at(refs.field.options, refs.value) >= 0 ? refs.field.options : refs.field.options.concat([refs.value])

        Layout.fillWidth: true
        visible: refs.kind === "ref"
        model: options.map(o => refs.label(o))
        currentIndex: refs.input.at(options, refs.value)
        onActivated: index => refs.input.commit(options[index])
    }

    Flow {
        Layout.fillWidth: true
        visible: refs.kind === "refs"

        Repeater {
            model: refs.kind === "refs" ? refs.field.options : []

            delegate: QQC2.CheckBox {
                required property var modelData

                text: String(modelData)
                checked: (refs.value ?? []).includes(modelData)
                onToggled: {
                    const now = refs.value ?? [];
                    refs.input.commit(refs.field.options.filter(o => o === modelData ? checked : now.includes(o)));
                }
            }
        }
    }

    RowLayout {
        visible: refs.kind === "map"

        QQC2.ComboBox {
            id: key

            Layout.fillWidth: true
            model: refs.field.options.filter(o => !refs.keys.includes(o))
        }

        QQC2.Button {
            text: App.tr("Add")
            icon.name: "list-add"
            enabled: key.currentText !== ""
            onClicked: refs.input.page.run("set", {
                "pointer": refs.field.pointer + "/" + key.currentText.replace(/~/g, "~0").replace(/\//g, "~1"),
                "value": refs.field.sample
            })
        }
    }
}
