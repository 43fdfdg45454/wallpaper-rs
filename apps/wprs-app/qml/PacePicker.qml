import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: picker

    required property var daemon
    readonly property string chosen: String((daemon.state?.options ?? {})["playback.auto_pace"] ?? "detail")
    readonly property var words: ({
            "detail": daemon.tr("Detail first: halve the frame rate before lowering the quality"),
            "smooth": daemon.tr("Smooth first: keep the frame rate and lower the quality")
        })
    readonly property var choices: Object.keys(words).map(value => ({
                    "value": value,
                    "text": words[value]
                }))

    QQC2.ComboBox {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 24
        Accessible.name: picker.daemon.tr("Automatic frame rate")
        model: picker.choices
        textRole: "text"
        valueRole: "value"
        currentIndex: Math.max(0, picker.choices.findIndex(c => c.value === picker.chosen))
        onActivated: picker.daemon.act("SetOption", ["playback.auto_pace", currentValue])
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        font: Kirigami.Theme.smallFont
        text: picker.daemon.tr("Applies to phones and to wprs-ctl scene bench; this computer keeps its own controller, so nothing changes here. With the automatic quality, a heavy scene may run at half the frame rate (never below 30) to keep a higher level.")
    }
}
