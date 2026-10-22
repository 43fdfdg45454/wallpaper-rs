import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tool

    required property var page
    readonly property var outputs: page.edit.outputs ?? {}
    readonly property var span: page.edit.span ?? {
        "center": [0.5, 0.5],
        "zoom": 1
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("A part for each screen")
    }

    Repeater {
        model: Array.from(new Set(App.outputs.concat(Object.keys(tool.outputs))))

        delegate: QQC2.Switch {
            required property string modelData
            readonly property var screen: App.state.outputs?.[modelData]

            Layout.fillWidth: true
            text: screen ? App.tr("%1 (%2×%3)", modelData, screen.width, screen.height) : modelData
            checked: modelData in tool.outputs
            onToggled: tool.page.run("crop", checked ? {
                "output": modelData,
                "aspect": screen ? screen.width + ":" + screen.height : undefined
            } : {
                "output": modelData,
                "rect": "none"
            })
        }
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Spread over the screens")
    }

    EditSlider {
        text: App.tr("Zoom")
        from: 1
        to: 8
        value: tool.span.zoom
        onMoved: value => tool.page.run("span", {
                "zoom": value
            })
    }

    Repeater {
        model: [App.tr("Horizontal center"), App.tr("Vertical center")]

        delegate: EditSlider {
            required property string modelData
            required property int index

            text: modelData
            value: tool.span.center[index]
            onMoved: value => {
                const center = tool.span.center.slice();
                center[index] = value;
                tool.page.run("span", {
                    "center": center.join(",")
                });
            }
        }
    }

    QQC2.Button {
        text: App.tr("Spread this picture over every screen")
        icon.name: "view-split-left-right"
        onClicked: App.act("SetWallpaper", [tool.page.item.path, "", "span"])
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: App.tr("The width of the screen frames is in the settings.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }
}
