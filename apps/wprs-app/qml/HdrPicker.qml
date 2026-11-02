import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

ColumnLayout {
    id: picker

    required property var daemon
    property string output: ""
    property var info: ({})
    readonly property var hdr: info.hdr ?? null
    readonly property string general: String((daemon.state?.options ?? {})["playback.hdr"] ?? "auto")
    readonly property string chosen: output ? (hdr?.own ?? "none") : general
    readonly property var words: ({
            "auto": daemon.tr("Automatic: on HDR screens, on mains power, for scenes with bright colors at the highest quality"),
            "on": daemon.tr("Always when the screen can show it, also on battery"),
            "off": daemon.tr("Never: standard colors only")
        })
    readonly property var reasons: ({
            "auto": daemon.tr("automatic"),
            "on": daemon.tr("chosen"),
            "off": daemon.tr("turned off"),
            "display": daemon.tr("this screen or the desktop offers no HDR"),
            "battery": daemon.tr("on battery"),
            "quality": daemon.tr("no scene with bright colors is shown at the highest quality")
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

    function support(h) {
        if (!h)
            return "";
        const state = h.active ? daemon.tr("HDR is on: %1", reasons[h.reason] ?? h.reason) : daemon.tr("HDR is off: %1", reasons[h.reason] ?? h.reason);
        const light = h.peak_nits ? daemon.tr("The screen reaches %1 nits and shows white at %2 nits.", h.peak_nits, h.reference_nits ?? "?") : "";
        return [state, light].filter(t => t).join(" ");
    }

    QQC2.ComboBox {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 24
        Accessible.name: picker.output ? picker.daemon.tr("HDR of %1", picker.output) : picker.daemon.tr("HDR")
        model: picker.choices
        textRole: "text"
        valueRole: "value"
        currentIndex: Math.max(0, picker.choices.findIndex(c => c.value === picker.chosen))
        onActivated: picker.daemon.act("SetOption", [picker.output ? "playback.hdr." + picker.output : "playback.hdr", currentValue])
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        visible: text !== ""
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        font: Kirigami.Theme.smallFont
        text: picker.output ? picker.support(picker.hdr) : picker.daemon.tr("Screens and desktops that support HDR show bright scenes brighter than white, tone mapped to how bright each screen gets. It uses more power, so the automatic choice keeps it to mains power and the highest scene quality.")
    }
}
