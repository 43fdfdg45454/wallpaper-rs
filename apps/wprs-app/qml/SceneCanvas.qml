import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: canvas

    required property var page
    readonly property var size: page.view.canvas?.size ?? [1920, 1080]
    readonly property real fitted: Math.min(area.width / size[0], area.height / size[1]) * 0.94
    readonly property real ratio: fitted * zoom
    property real zoom: 1
    property point pan: Qt.point(0, 0)
    property var stream: null
    property bool missing: false
    readonly property var aspects: ["", "16:9", "21:9", "32:9", "4:3", "16:10", "9:16"]

    function connect() {
        stream?.destroy();
        stream = null;
        if (App.checking || page.editor.node === 0)
            return;
        try {
            stream = Qt.createQmlObject("import QtQuick; import org.kde.pipewire as PipeWire; PipeWire.PipeWireSourceItem { anchors.fill: parent }", screen);
            if (page.editor.serial > 0 && stream.objectSerial !== undefined)
                stream.objectSerial = page.editor.serial;
            else
                stream.nodeId = page.editor.node;
        } catch (error) {
            missing = true;
        }
    }

    function guide(aspect) {
        const parts = aspect.split(":").map(Number);
        const ratio = parts[0] / parts[1], own = size[0] / size[1];
        return ratio > own ? [size[0], size[0] / ratio] : [size[1] * ratio, size[1]];
    }

    spacing: 0

    Connections {
        target: canvas.page.editor

        function onSerialChanged() {
            canvas.connect();
        }
    }

    QQC2.ToolBar {
        Layout.fillWidth: true

        RowLayout {
            anchors.fill: parent

            QQC2.ToolButton {
                icon.name: "zoom-out"
                text: App.tr("Zoom out")
                display: QQC2.AbstractButton.IconOnly
                onClicked: canvas.zoom = Math.max(0.1, canvas.zoom / 1.25)
            }

            QQC2.Label {
                text: Math.round(canvas.zoom * 100) + " %"
            }

            QQC2.ToolButton {
                icon.name: "zoom-in"
                text: App.tr("Zoom in")
                display: QQC2.AbstractButton.IconOnly
                onClicked: canvas.zoom = Math.min(16, canvas.zoom * 1.25)
            }

            QQC2.ToolButton {
                icon.name: "zoom-fit-best"
                text: App.tr("Fit")
                display: QQC2.AbstractButton.IconOnly
                onClicked: {
                    canvas.zoom = 1;
                    canvas.pan = Qt.point(0, 0);
                }
            }

            Item {
                Layout.fillWidth: true
            }

            QQC2.Label {
                text: App.tr("Guide:")
            }

            QQC2.ComboBox {
                id: guides

                model: canvas.aspects.map(aspect => aspect || App.tr("No guide"))
            }
        }
    }

    Rectangle {
        id: area

        Layout.fillWidth: true
        Layout.fillHeight: true
        color: Kirigami.Theme.alternateBackgroundColor
        clip: true

        DragHandler {
            property point start

            target: null
            acceptedButtons: Qt.MiddleButton | Qt.RightButton
            onActiveChanged: start = canvas.pan
            onTranslationChanged: canvas.pan = Qt.point(start.x + translation.x, start.y + translation.y)
        }

        WheelHandler {
            onWheel: wheel => {
                if (wheel.modifiers & Qt.ControlModifier)
                    canvas.zoom = Math.max(0.1, Math.min(16, canvas.zoom * (wheel.angleDelta.y > 0 ? 1.1 : 1 / 1.1)));
                else
                    canvas.pan = Qt.point(canvas.pan.x + wheel.angleDelta.x / 4, canvas.pan.y + wheel.angleDelta.y / 4);
            }
        }

        TapHandler {
            onTapped: canvas.page.editor.select("/scene/canvas")
        }

        SceneBand {
            anchors.fill: parent
            enabled: !canvas.page.moving
            page: canvas.page
            screen: screen
            ratio: canvas.ratio
        }

        Rectangle {
            id: screen

            x: (area.width - width) / 2 + canvas.pan.x
            y: (area.height - height) / 2 + canvas.pan.y
            width: canvas.size[0] * canvas.ratio
            height: canvas.size[1] * canvas.ratio
            color: canvas.page.view.canvas?.background ?? "black"
            border.color: Kirigami.Theme.disabledTextColor

            QQC2.Label {
                anchors.centerIn: parent
                width: parent.width * 0.8
                visible: canvas.stream === null
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                color: "white"
                style: Text.Outline
                text: canvas.missing ? App.tr("Install the KPipeWire QML module to see the preview here, or use Show on the desktop.") : App.tr("The preview appears here while wprsd runs.")
            }

            SceneWind {
                anchors.fill: parent
                z: 4
                page: canvas.page
                ratio: canvas.ratio
            }

            Repeater {
                model: (canvas.page.view.gizmos ?? []).length

                delegate: SceneGizmo {
                    page: canvas.page
                    ratio: canvas.ratio
                }
            }

            Rectangle {
                readonly property var box: guides.currentIndex > 0 ? canvas.guide(canvas.aspects[guides.currentIndex]) : [0, 0]

                anchors.centerIn: parent
                z: 3
                visible: guides.currentIndex > 0
                width: box[0] * canvas.ratio
                height: box[1] * canvas.ratio
                color: "transparent"
                border.color: Kirigami.Theme.highlightColor
                border.width: 2
            }
        }
    }
}
