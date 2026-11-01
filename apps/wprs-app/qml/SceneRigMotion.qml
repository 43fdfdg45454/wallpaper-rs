import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: motion

    required property var host
    property int revision: 0
    readonly property var names: host.bones.map(b => b.name)
    readonly property var chains: (revision, listed("/ik", []))
    readonly property var bodies: (revision, listed("/physics", []))
    readonly property var touched: (revision, listed("/hit", []))
    readonly property var depth: (revision, listed("/depth", null))
    readonly property var targets: ["cursor"].concat(names.map(n => "bone:" + n))

    function listed(key, empty) {
        return JSON.parse(host.input.page.editor.value(host.base + key) || "null") ?? empty;
    }

    function put(key, value) {
        if (value === null)
            host.input.page.run("remove", {
                "pointer": host.base + key
            });
        else
            host.input.page.run("set", {
                "pointer": host.base + key,
                "value": value
            });
    }

    function chainTo(end, count) {
        const chain = [end];
        let at = host.bones.find(b => b.name === end);
        while (chain.length < count && at && at.parent) {
            chain.unshift(at.parent);
            at = host.bones.find(b => b.name === at.parent);
        }
        return chain;
    }

    function aim(target) {
        if (target === "cursor")
            return App.tr("the pointer");
        return typeof target === "string" ? target.replace(/^bone:/, "") : App.tr("a point");
    }

    function without(list, index) {
        return list.filter((_, i) => i !== index);
    }

    Connections {
        function onViewChanged() {
            motion.revision++;
        }

        target: motion.host.input.page.editor
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    QQC2.Label {
        text: App.tr("Chains that reach")
        font.bold: true
    }

    Repeater {
        model: motion.chains

        delegate: SceneRigRow {
            required property var modelData
            required property int index

            text: App.tr("%1 reach for %2", modelData.chain.join(" → "), motion.aim(modelData.target))
            onRemoved: motion.put("/ik", motion.without(motion.chains, index))
        }
    }

    RowLayout {
        QQC2.ComboBox {
            id: end

            Layout.fillWidth: true
            model: motion.names
        }

        QQC2.SpinBox {
            id: count

            from: 2
            to: 8
            value: 2
        }

        QQC2.ComboBox {
            id: target

            Layout.fillWidth: true
            model: [App.tr("The pointer"), App.tr("A point")].concat(motion.names)
        }

        QQC2.Button {
            text: App.tr("Add")
            enabled: motion.chainTo(end.currentText, count.value).length >= 2
            onClicked: motion.put("/ik", motion.chains.concat([{
                    "chain": motion.chainTo(end.currentText, count.value),
                    "target": target.currentIndex === 0 ? "cursor" : target.currentIndex === 1 ? [0, 0] : "bone:" + motion.names[target.currentIndex - 2]
                }]))
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: target.currentIndex === 0 || motion.chains.some(c => c.target === "cursor")
        wrapMode: Text.Wrap
        text: App.tr("A chain that follows the pointer needs the pointer permission, and the scene tells the user that it follows the pointer.")
    }

    QQC2.Label {
        text: App.tr("Bones that swing")
        font.bold: true
    }

    Repeater {
        model: motion.bodies

        delegate: SceneRigRow {
            required property var modelData
            required property int index

            text: App.tr("%1 swings as a %2", modelData.bone, SceneWords.value(modelData.mode ?? "spring"))
            onRemoved: motion.put("/physics", motion.without(motion.bodies, index))
        }
    }

    RowLayout {
        QQC2.ComboBox {
            id: swinging

            Layout.fillWidth: true
            model: motion.names.filter(n => !motion.bodies.some(b => b.bone === n))
        }

        QQC2.ComboBox {
            id: mode

            readonly property var modes: ["spring", "rigid", "rope"]

            model: modes.map(m => SceneWords.value(m))
        }

        QQC2.Button {
            text: App.tr("Add")
            enabled: swinging.currentText !== ""
            onClicked: motion.put("/physics", motion.bodies.concat([{
                    "bone": swinging.currentText,
                    "mode": mode.modes[mode.currentIndex]
                }]))
        }
    }

    SceneRigTouch {
        Layout.fillWidth: true
        owner: motion
    }
}
