import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: picker

    required property var daemon
    property string output: ""
    property var info: ({})
    readonly property string general: String((daemon.state?.options ?? {})["playback.quality"] ?? "auto")
    readonly property string chosen: output ? (info.quality_choice ?? "none") : general
    readonly property var words: ({
            "auto": daemon.tr("Automatic: lower on battery, when hot or when the scene is heavy"),
            "min-q2": daemon.tr("Automatic, but never below high (2 of 3)"),
            "min-q1": daemon.tr("Automatic, but never below reduced (1 of 3)"),
            "q3": daemon.tr("Always the highest (3 of 3)"),
            "q2": daemon.tr("Always high (2 of 3)"),
            "q1": daemon.tr("Always reduced (1 of 3)"),
            "q0": daemon.tr("Always the lowest (0 of 3), uses the least power")
        })
    readonly property var choices: {
        const found = Object.keys(words).map(value => ({
                    "value": value,
                    "text": words[value]
                }));
        if (output)
            found.unshift({
                "value": "none",
                "text": daemon.tr("Like every screen: %1", words[general] ?? general)
            });
        return found;
    }

    QQC2.ComboBox {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 24
        Accessible.name: picker.output ? picker.daemon.tr("Scene quality of %1", picker.output) : picker.daemon.tr("Scene quality")
        model: picker.choices
        textRole: "text"
        valueRole: "value"
        currentIndex: Math.max(0, picker.choices.findIndex(c => c.value === picker.chosen))
        onActivated: picker.daemon.act("SetOption", [picker.output ? "playback.quality." + picker.output : "playback.quality", currentValue])
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        visible: !picker.output
        wrapMode: Text.Wrap
        text: picker.daemon.tr("A level you choose holds on battery, when the computer is hot and for heavy scenes too; a scene heavier than this graphics card holds then shows fewer frames per second.")
    }
}
