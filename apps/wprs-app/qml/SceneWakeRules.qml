import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: rules

    required property var host
    readonly property var info: host.field.value ?? ({})
    readonly property var names: (info.params ?? []).map(p => p.id)
    readonly property var inputs: [["", App.tr("Nothing")], ["time", App.tr("Time")], ["cursor", App.tr("The pointer")], ["audio", App.tr("The sound")], ["clicks", App.tr("Clicks")]]

    function change(param, key, value) {
        const args = {
            "effect": info.effect,
            "param": param
        };
        args[key] = value ?? "";
        host.page.run("effect_rule", args);
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: rules.info.own ? App.tr("Each parameter may ask for an input while it is not zero, wait for another parameter or yield to one; the scene sleeps when none of them asks.") : App.tr("A built-in effect keeps these rules; an effect of the package changes them here.")
    }

    QQC2.Label {
        visible: (rules.info.uses ?? []).length > 0
        text: App.tr("Always asks for: %1", (rules.info.uses ?? []).join(", "))
    }

    Repeater {
        model: rules.info.params ?? []

        delegate: RowLayout {
            id: row

            required property var modelData
            readonly property var others: [""].concat(rules.names.filter(n => n !== modelData.id))

            QQC2.Label {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                text: row.modelData.id
                elide: Text.ElideRight
            }

            QQC2.ComboBox {
                enabled: rules.info.own === true
                model: rules.inputs.map(i => i[1])
                currentIndex: Math.max(0, rules.inputs.findIndex(i => i[0] === (row.modelData.uses ?? "")))
                onActivated: index => rules.change(row.modelData.id, "uses", rules.inputs[index][0])
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: App.tr("Asks for")
            }

            QQC2.ComboBox {
                enabled: rules.info.own === true
                model: row.others.map(n => n === "" ? App.tr("Waits for nothing") : App.tr("Waits for %1", n))
                currentIndex: Math.max(0, row.others.indexOf(row.modelData.waits_for ?? ""))
                onActivated: index => rules.change(row.modelData.id, "waits_for", row.others[index])
            }

            QQC2.ComboBox {
                enabled: rules.info.own === true
                model: row.others.map(n => n === "" ? App.tr("Yields to nothing") : App.tr("Yields to %1", n))
                currentIndex: Math.max(0, row.others.indexOf(row.modelData.yields_to ?? ""))
                onActivated: index => rules.change(row.modelData.id, "yields_to", row.others[index])
            }
        }
    }
}
