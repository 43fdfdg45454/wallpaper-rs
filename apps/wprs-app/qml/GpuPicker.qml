import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: picker

    readonly property var adapters: App.state.adapters ?? []
    readonly property string chosen: String((App.state.options ?? {})["render.gpu"] ?? "auto")
    readonly property var kinds: ({
            "integrated": App.tr("integrated"),
            "dedicated": App.tr("dedicated"),
            "virtual": App.tr("virtual"),
            "software": App.tr("software"),
            "other": App.tr("other kind")
        })
    readonly property var words: ({
            "auto": App.tr("Automatic, the one that saves the most power"),
            "integrated": App.tr("Any integrated card"),
            "dedicated": App.tr("Any dedicated card"),
            "software": App.tr("Software rendering, for tests")
        })
    readonly property var choices: {
        const found = [{
                "value": "auto",
                "text": words.auto
            }];
        for (const kind of ["integrated", "dedicated", "software"]) {
            if (adapters.some(a => a.kind === kind) || chosen === kind)
                found.push({
                        "value": kind,
                        "text": words[kind]
                    });
        }
        for (const card of adapters) {
            const same = adapters.filter(a => a.name === card.name);
            if (found.some(c => c.value === card.name))
                continue;
            found.push({
                    "value": card.name,
                    "text": label(card.name, same)
                });
        }
        if (!found.some(c => c.value.toLowerCase() === chosen.toLowerCase()))
            found.push({
                    "value": chosen,
                    "text": adapters.some(a => a.name.toLowerCase().includes(chosen.toLowerCase())) ? App.tr("The first card whose name has “%1”", chosen) : App.tr("“%1”, which no card matches", chosen)
                });
        return found;
    }

    function backend(name) {
        return ({
                "vulkan": "Vulkan",
                "gl": "OpenGL ES"
            })[name] ?? name;
    }

    function label(name, same) {
        const used = same.some(a => a.used) ? App.tr("in use") : "";
        const ways = same.map(a => backend(a.backend)).join(", ");
        return [name, kinds[same[0].kind] ?? same[0].kind, ways, used].filter(t => t).join(" · ");
    }

    QQC2.ComboBox {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 24
        Accessible.name: App.tr("Graphics card")
        model: picker.choices
        textRole: "text"
        valueRole: "value"
        currentIndex: Math.max(0, picker.choices.findIndex(c => c.value.toLowerCase() === picker.chosen.toLowerCase()))
        onActivated: App.act("SetOption", ["render.gpu", currentValue])
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: [App.tr("A dedicated card draws heavy scenes at a higher quality and uses more power."), picker.adapters.filter(a => a.used).map(a => App.tr("In use: %1 (%2).", a.name, picker.backend(a.backend))).join(" ")].filter(t => t).join(" ")
    }

    QQC2.Label {
        visible: App.state.gpu_restart === true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: App.tr("The new choice takes effect the next time the daemon starts.")
        color: Kirigami.Theme.neutralTextColor
    }

    QQC2.Label {
        visible: !!App.state.gpu_warning
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: App.state.gpu_warning ?? ""
        color: Kirigami.Theme.negativeTextColor
    }
}
