import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

GridLayout {
    id: light

    required property var tools
    readonly property var info: SceneSwarmGlow.info?.system === tools.system ? SceneSwarmGlow.info : null
    readonly property var pictures: [App.tr("None")].concat(info?.pictures ?? [])

    function load() {
        SceneSwarmGlow.load(tools.page, tools.system, null);
    }

    columns: 2
    visible: info !== null
    Component.onCompleted: load()

    Connections {
        function onInfoChanged() {
            light.load();
        }

        target: light.tools
    }

    Kirigami.Heading {
        Layout.columnSpan: 2
        level: 4
        text: App.tr("Light")
    }

    QQC2.CheckBox {
        Layout.columnSpan: 2
        text: App.tr("Take the lights of the scene")
        checked: light.info?.lit === true
        onToggled: SceneSwarmGlow.run(light.tools.page, "lit", {
            "lit": checked
        })
    }

    QQC2.Label {
        text: App.tr("Normal map")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: light.pictures
        currentIndex: Math.max(0, light.pictures.indexOf(light.info?.normal ?? ""))
        onActivated: index => SceneSwarmGlow.run(light.tools.page, "lit", {
                "normal": index === 0 ? "" : light.pictures[index]
            })
    }

    Image {
        Layout.columnSpan: 2
        visible: light.info?.lit === true
        source: light.info?.sides ?? ""
        sourceSize.width: 96
        sourceSize.height: 48
    }

    QQC2.Label {
        Layout.columnSpan: 2
        Layout.fillWidth: true
        visible: light.info?.lit === true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Lit from the front and from behind: each sprite takes up to 8 lights on both faces.")
    }

    SceneSwarmBend {
        Layout.columnSpan: 2
        Layout.fillWidth: true
        light: light
    }
}
