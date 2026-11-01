import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Kirigami.FormLayout {
    id: form

    required property var host
    readonly property var current: host.info.filter ?? ({})
    readonly property var looks: ["none", "warm", "cool", "golden", "vivid", "punch", "muted", "matte", "fade", "mono", "noir", "sepia", "dusk", "mint", "rose", "night"]
    readonly property var names: [host.daemon.tr("None"), host.daemon.tr("Warm"), host.daemon.tr("Cool"), host.daemon.tr("Golden"), host.daemon.tr("Vivid"), host.daemon.tr("Punch"), host.daemon.tr("Subdued"), host.daemon.tr("Matte"), host.daemon.tr("Faded"), host.daemon.tr("Black and white"), host.daemon.tr("Noir"), host.daemon.tr("Sepia"), host.daemon.tr("Dusk"), host.daemon.tr("Mint"), host.daemon.tr("Rose"), host.daemon.tr("Night")]
    readonly property var sliders: [["strength", host.daemon.tr("Strength"), 0, 1, 1], ["exposure", host.daemon.tr("Exposure"), -3, 3, 0], ["contrast", host.daemon.tr("Contrast"), -1, 1, 0], ["saturation", host.daemon.tr("Saturation"), -1, 1, 0], ["warmth", host.daemon.tr("Warmth"), -1, 1, 0], ["hue", host.daemon.tr("Hue"), -180, 180, 0]]
    property var chosen: ({})

    function send() {
        host.daemon.act("SetFilter", [JSON.stringify(Object.assign({}, current, chosen)), host.name]);
    }

    QQC2.ComboBox {
        Kirigami.FormData.label: host.daemon.tr("Filter:")
        model: form.names
        currentIndex: Math.max(0, form.looks.indexOf(form.current.look ?? "none"))
        onActivated: index => {
            form.chosen = Object.assign({}, form.chosen, {
                "look": form.looks[index]
            });
            form.send();
        }
    }

    Repeater {
        model: form.sliders

        QQC2.Slider {
            required property var modelData

            Kirigami.FormData.label: modelData[1] + ":"
            from: modelData[2]
            to: modelData[3]
            stepSize: (modelData[3] - modelData[2]) / 120
            value: form.current[modelData[0]] ?? modelData[4]
            onMoved: {
                form.chosen = Object.assign({}, form.chosen, {
                    [modelData[0]]: value
                });
                later.restart();
            }
        }
    }

    QQC2.Button {
        text: host.daemon.tr("Show as it is")
        icon.name: "edit-reset"
        enabled: host.info.filter !== null && host.info.filter !== undefined
        onClicked: {
            form.chosen = {};
            host.daemon.act("SetFilter", ["", host.name]);
        }
    }

    Timer {
        id: later

        interval: 200
        onTriggered: form.send()
    }
}
