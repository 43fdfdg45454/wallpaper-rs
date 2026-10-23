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
    readonly property var both: Array.isArray(value) ? value : [Number(value ?? 0), Number(value ?? 0)]

    function commit(value, gesture) {
        page.change(field.pointer, value, gesture);
    }

    function parse(text, fallback) {
        const parsed = Number(String(text).replace(",", "."));
        return Number.isFinite(parsed) ? parsed : fallback;
    }

    function name(option) {
        if (field.widget === "ref:language")
            return ManifestWords.language(option);
        if (["enum", "variant", "ref:effect"].includes(field.widget))
            return SceneWords.value(option);
        return (field.names ?? [])[field.options.indexOf(option)] || (field.widget === "ref:trigger" ? SceneWords.label(option) : String(option));
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
        text: App.tr("Follows %1").arg(SceneWords.binding(input.field.value))
        icon.name: "document-edit"
        flat: true
        Layout.fillWidth: true
        onClicked: input.page.editor.select(input.field.pointer)
    }

    Loader {
        Layout.fillWidth: true
        active: !input.bound && input.field.widget !== "group"
        sourceComponent: ({
                "number": numberEditor,
                "integer": numberEditor,
                "bool": checkEditor,
                "enum": choiceEditor,
                "variant": choiceEditor,
                "color": colorEditor,
                "rgb": colorEditor,
                "condition": conditionEditor,
                "vec2": pairEditor,
                "curve": curveEditor,
                "stops": pairsEditor,
                "points": pairsEditor,
                "list": listEditor,
                "texts": textsEditor,
                "markup": textsEditor,
                "flags": metaEditor,
                "lines": metaEditor,
                "computed": metaEditor,
                "ref:thumbnail": metaEditor,
                "ref:animated": metaEditor
            })[input.field.widget] ?? (input.field.widget.startsWith("ref:") ? choiceEditor : rawEditor)
    }

    Component {
        id: numberEditor

        RowLayout {
            id: amount

            readonly property bool named: input.field.options.includes(input.value)

            QQC2.Slider {
                Layout.fillWidth: true
                visible: Array.isArray(input.field.range) && !amount.named
                from: (input.field.range ?? [0, 1])[0]
                to: (input.field.range ?? [0, 1])[1]
                stepSize: input.field.widget === "integer" ? 1 : 0
                value: Number(input.value ?? 0)
                onMoved: input.commit(Math.round(value * 1000) / 1000, true)
            }

            QQC2.TextField {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 5
                Layout.fillWidth: !Array.isArray(input.field.range)
                visible: !amount.named
                text: typeof input.value === "number" ? String(Number(input.value.toFixed(4))) : ""
                onEditingFinished: input.commit(input.parse(text, input.value))
            }

            Repeater {
                model: input.field.options

                delegate: QQC2.CheckBox {
                    required property var modelData

                    Layout.fillWidth: amount.named
                    text: SceneWords.value(modelData)
                    checked: input.value === modelData
                    onToggled: input.commit(checked ? modelData : typeof input.field.default === "number" ? input.field.default : 0)
                }
            }
        }
    }

    Component {
        id: checkEditor

        RowLayout {
            id: toggle

            readonly property bool unset: input.field.required && !input.field.set

            QQC2.Switch {
                visible: !toggle.unset
                checked: input.value === true
                onToggled: input.commit(checked)
            }

            QQC2.Label {
                visible: toggle.unset
                text: App.tr("Not set")
                color: Kirigami.Theme.negativeTextColor
            }

            Repeater {
                model: toggle.unset ? [false, true] : []

                delegate: QQC2.Button {
                    required property bool modelData

                    text: modelData ? App.tr("Yes") : App.tr("No")
                    onClicked: input.commit(modelData)
                }
            }
        }
    }

    Component {
        id: choiceEditor

        QQC2.ComboBox {
            readonly property bool variant: input.field.widget === "variant"
            readonly property var known: variant || input.field.required ? input.field.options : [null].concat(input.field.options)
            readonly property var options: known.includes(input.value) || input.value === null ? known : known.concat([input.value])

            model: options.map(option => option === null ? App.tr("Not set") : input.name(option))
            currentIndex: variant ? options.findIndex(option => Object.keys(input.field.value ?? {}).includes(option) || option === input.field.value) : options.indexOf(input.value)
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

    Component {
        id: pairsEditor

        ScenePairs {
            host: input
        }
    }

    Component {
        id: pairEditor

        RowLayout {
            Repeater {
                model: 2

                delegate: QQC2.TextField {
                    required property int index

                    Layout.fillWidth: true
                    text: String(Number(Number(input.both[index]).toFixed(4)))
                    onEditingFinished: {
                        const next = [input.both[0], input.both[1]];
                        next[index] = input.parse(text, next[index]);
                        input.commit(next);
                    }
                }
            }
        }
    }

    Component {
        id: listEditor

        RowLayout {
            QQC2.Label {
                Layout.fillWidth: true
                text: (input.value ?? []).length === 1 ? App.tr("1 item") : App.tr("%1 items").arg((input.value ?? []).length)
            }

            QQC2.Button {
                icon.name: "list-add"
                text: App.tr("Add")
                onClicked: input.page.run(input.field.set ? "add" : "set", {
                        "pointer": input.field.pointer + (input.field.set ? "/-" : ""),
                        "value": input.field.set ? input.field.sample : [input.field.sample]
                    })
            }

            QQC2.ComboBox {
                model: (input.value ?? []).map((_, i) => App.tr("Item %1").arg(i + 1))
                displayText: App.tr("Edit…")
                enabled: count > 0
                onActivated: index => input.page.editor.select(input.field.pointer + "/" + index)
            }
        }
    }

    Component {
        id: curveEditor

        ColumnLayout {
            Canvas {
                id: plot

                readonly property var points: Array.isArray(input.value) ? input.value : []

                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 3
                onPointsChanged: requestPaint()
                onPaint: {
                    const context = getContext("2d");
                    context.reset();
                    context.strokeStyle = Kirigami.Theme.highlightColor;
                    context.lineWidth = 2;
                    const values = points.map(p => Number(p[1]) || 0), top = Math.max(1, ...values);
                    context.beginPath();
                    points.forEach((p, i) => context[i ? "lineTo" : "moveTo"](p[0] * width, height - (Number(p[1]) || 0) / top * (height - 4) - 2));
                    context.stroke();
                }
            }

            QQC2.TextField {
                Layout.fillWidth: true
                text: JSON.stringify(input.value ?? [])
                onEditingFinished: {
                    try {
                        input.commit(JSON.parse(text));
                    } catch (error) {
                        input.page.editor.error = App.tr("%1 is not valid JSON").arg(text);
                    }
                }
            }
        }
    }

    Component {
        id: conditionEditor

        SceneCondition {
            host: input
        }
    }

    Component {
        id: textsEditor

        SceneTexts {
            host: input
        }
    }

    Component {
        id: metaEditor

        SceneMeta {
            host: input
        }
    }

    Component {
        id: rawEditor

        QQC2.TextField {
            readonly property bool plain: ["text", "scalar"].includes(input.field.widget) || input.field.widget.startsWith("ref:")

            text: plain ? String(input.value ?? "") : JSON.stringify(input.value ?? null)
            onEditingFinished: {
                const number = Number(text.trim());
                if (input.field.widget === "scalar")
                    return input.commit(["true", "false"].includes(text.trim()) ? text.trim() === "true" : text.trim() !== "" && Number.isFinite(number) ? number : text);
                if (input.field.widget === "text")
                    return input.commit(text);
                try {
                    input.commit(JSON.parse(text));
                } catch (error) {
                    input.page.editor.error = App.tr("%1 is not valid JSON").arg(text);
                }
            }
        }
    }
}
