import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: input

    required property var page
    required property var field
    readonly property var value: field.shown
    readonly property bool bound: field.source !== "" && field.source !== "keys"
    readonly property var both: typeof value === "object" && value !== null && value.length >= 2 ? [Number(value[0]), Number(value[1])] : [Number(value ?? 0), Number(value ?? 0)]

    function commit(value, gesture) {
        page.change(field.pointer, value, gesture);
    }

    function parse(text, fallback) {
        const parsed = Number(String(text).replace(",", "."));
        return Number.isFinite(parsed) ? parsed : fallback;
    }

    function at(list, option) {
        return SceneOptions.at(list, option);
    }

    function name(option) {
        return SceneOptions.name(field, option);
    }

    Layout.leftMargin: field.depth * Kirigami.Units.largeSpacing

    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: 2
        visible: input.page.spot === input.field.pointer
        color: Kirigami.Theme.negativeTextColor
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Layout.fillWidth: true
            text: (input.field.title || SceneWords.label(input.field.key, input.field.pointer)) + (input.field.unit ? " (" + input.field.unit + ")" : "")
            font.bold: input.field.widget === "group" || input.field.widget === "variant"
            elide: Text.ElideRight
            QQC2.ToolTip.text: input.field.help ?? ""
            QQC2.ToolTip.visible: hint.hovered && (input.field.help ?? "") !== ""

            HoverHandler {
                id: hint
            }
        }

        QQC2.ToolButton {
            visible: input.field.widget === "group" && !input.field.set
            icon.name: "list-add"
            text: App.tr("Add")
            onClicked: input.commit(input.field.sample)
        }

        QQC2.ToolButton {
            visible: input.field.set && !input.field.required && input.field.widget !== "computed"
            icon.name: "edit-reset"
            text: input.field.widget === "group" ? App.tr("Remove") : App.tr("Default")
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: input.page.run("remove", {
                "pointer": input.field.pointer
            })
        }

        SceneBind {
            visible: input.field.bindable
            page: input.page
            field: input.field
        }
    }

    QQC2.Button {
        visible: input.bound
        text: App.tr("Follows %1", SceneWords.binding(input.field.value))
        icon.name: "document-edit"
        flat: true
        Layout.fillWidth: true
        onClicked: input.page.editor.select(input.field.pointer)
    }

    SceneEditors {
        id: editors

        input: parent
    }

    SceneHosts {
        id: hosts

        input: parent
    }

    Loader {
        Layout.fillWidth: true
        active: !input.bound && input.field.widget !== "group"
        sourceComponent: ({
                "number": editors.number,
                "integer": editors.number,
                "bool": editors.check,
                "enum": choiceEditor,
                "variant": choiceEditor,
                "color": colorEditor,
                "rgb": colorEditor,
                "condition": hosts.condition,
                "vec2": editors.pair,
                "curve": editors.curve,
                "stops": hosts.pairs,
                "points": hosts.pairs,
                "styles": hosts.styles,
                "emitters": hosts.emitters,
                "rules": hosts.rules,
                "list": editors.list,
                "ref:mask": editors.mask,
                "ref:model": editors.rig,
                "texts": hosts.texts,
                "markup": hosts.texts,
                "flags": hosts.meta,
                "lines": hosts.meta,
                "computed": hosts.meta,
                "ref:thumbnail": hosts.meta,
                "ref:animated": hosts.meta
            })[input.field.widget] ?? (/^(refs|map):|^ref:reach$/.test(input.field.widget) ? editors.refs : input.field.widget.startsWith("ref:") ? choiceEditor : editors.raw)
    }

    Component {
        id: choiceEditor

        QQC2.ComboBox {
            readonly property bool variant: input.field.widget === "variant"
            readonly property var known: variant || input.field.required ? input.field.options : [null].concat(input.field.options)
            readonly property var options: input.at(known, input.value) >= 0 || input.value === null ? known : known.concat([input.value])

            model: options.map(option => option === null ? App.tr("Not set") : input.name(option))
            currentIndex: variant ? options.findIndex(option => Object.keys(input.field.value ?? {}).includes(option) || option === input.field.value) : input.at(options, input.value)
            onActivated: index => {
                if (options[index] === null)
                    input.page.run("remove", {
                        "pointer": input.field.pointer
                    });
                else
                    input.commit(variant ? input.field.sample[options[index]] : options[index]);
            }
        }
    }

    Component {
        id: colorEditor

        SceneColor {
            alpha: input.field.widget !== "rgb"
            hex: typeof input.value === "string" ? input.value : "#ffffff"
            onPicked: hex => input.commit(hex)
        }
    }
}
