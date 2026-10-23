import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tool

    required property var page
    readonly property var depth: page.values.depth ?? {}
    readonly property var stages: ({
            "download": App.tr("Downloading the model…"),
            "loading": App.tr("Loading the model…"),
            "estimating": App.tr("Estimating depth…"),
            "saving": App.tr("Saving the map…")
        })

    DepthMaps {
        id: maps

        Component.onCompleted: open(tool.page.item.path ?? "")
        onMapChanged: {
            if (map !== "")
                tool.page.editor.refresh();
        }
    }

    Image {
        Layout.fillWidth: true
        Layout.preferredHeight: width * 0.6
        source: maps.map
        cache: false
        asynchronous: true
        fillMode: Image.PreserveAspectFit
        visible: maps.map !== ""
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: maps.map === "" && !maps.busy
        text: App.tr("This picture has no depth map yet. White is near and black is far.")
        wrapMode: Text.Wrap
    }

    RowLayout {
        visible: maps.busy

        QQC2.ProgressBar {
            Layout.fillWidth: true
            value: Math.max(0, maps.progress)
            indeterminate: maps.progress < 0
        }

        QQC2.ToolButton {
            icon.name: "process-stop"
            text: App.tr("Cancel")
            onClicked: maps.cancel()
        }
    }

    QQC2.Label {
        visible: maps.busy
        text: tool.stages[maps.stage] ?? ""
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        type: Kirigami.MessageType.Error
        text: maps.error
        visible: text !== ""
    }

    QQC2.Button {
        text: App.tr("Generate the map")
        icon.name: "view-preview"
        enabled: !maps.busy
        onClicked: maps.ready ? maps.generate(tool.depth.input ?? 518) : consent.open()
    }

    QQC2.Button {
        text: App.tr("Import a map…")
        icon.name: "document-import"
        enabled: !maps.busy
        onClicked: files.open()
    }

    QQC2.Button {
        text: App.tr("Save next to the picture")
        icon.name: "document-save"
        enabled: maps.map !== "" && !maps.busy
        onClicked: maps.keep()
    }

    Repeater {
        model: [["near", App.tr("Near level"), 0, 1, 0.01], ["far", App.tr("Far level"), 0, 1, 0.01], ["gamma", App.tr("Curve"), 0.2, 5, 0.05], ["smooth", App.tr("Smoothing (px)"), 0, 16, 1], ["dilate", App.tr("Grow near edges (px)"), 0, 16, 1]]

        delegate: EditSlider {
            required property var modelData

            text: modelData[1]
            from: modelData[2]
            to: modelData[3]
            stepSize: modelData[4]
            value: tool.depth[modelData[0]] ?? 0
            onMoved: value => {
                const args = {};
                args[modelData[0]] = value;
                tool.page.run("depth", args);
            }
        }
    }

    FileDialog {
        id: files

        nameFilters: [App.tr("Pictures") + " (*.png *.jpg *.jpeg *.webp *.bmp)"]
        onAccepted: maps.adopt(selectedFile.toString())
    }

    DepthDialog {
        id: consent

        maps: maps
    }
}
