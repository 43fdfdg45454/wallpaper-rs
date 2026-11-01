import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: air

    required property var host
    readonly property var page: host.page
    readonly property var mesh: page.mesh
    property var wind: ({})

    function load() {
        wind = mesh.ask("", {
            "wind": true
        }) ?? {};
    }

    function blow(args) {
        page.run("wind", args, true);
    }

    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            air.load();
        }

        target: air.page.editor
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    RowLayout {
        QQC2.Label {
            Layout.fillWidth: true
            text: App.tr("Wind")
            font.bold: true
        }

        QQC2.Button {
            text: App.tr("Show on the preview")
            icon.name: "weather-windy"
            checkable: true
            checked: air.mesh.mode === "wind"
            onToggled: air.mesh.use(air.host.layerId, checked ? "wind" : "")
        }

        QQC2.ToolButton {
            icon.name: "edit-clear"
            text: App.tr("No wind")
            display: QQC2.AbstractButton.IconOnly
            enabled: air.wind.strength > 0
            onClicked: air.page.run("wind", {
                "off": true
            })
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: air.wind.fixed === false
        wrapMode: Text.Wrap
        text: App.tr("The wind follows a binding or keyframes; set it here to make it fixed again.")
    }

    GridLayout {
        columns: 3

        QQC2.Label {
            text: App.tr("Angle")
        }

        QQC2.Slider {
            id: angle

            Layout.fillWidth: true
            from: -180
            to: 180
            value: air.wind.angle ?? 0
            onMoved: air.blow({
                "angle": Math.round(value)
            })
        }

        QQC2.Label {
            text: Math.round(angle.value) + "°"
        }

        QQC2.Label {
            text: App.tr("Strength")
        }

        QQC2.Slider {
            id: strength

            Layout.fillWidth: true
            from: 0
            to: 1000
            value: air.wind.strength ?? 0
            onMoved: air.blow({
                "strength": Math.round(value)
            })
        }

        QQC2.Label {
            text: App.tr("%1 px/s", Math.round(strength.value))
        }

        QQC2.Label {
            text: App.tr("Gusts")
        }

        QQC2.Slider {
            id: gusts

            Layout.fillWidth: true
            value: air.wind.gusts ?? 0.35
            onMoved: air.blow({
                "gusts": Math.round(value * 100) / 100
            })
        }

        QQC2.Label {
            text: Math.round(gusts.value * 100) + " %"
        }
    }
}
