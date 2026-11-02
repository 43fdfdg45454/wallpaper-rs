import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: span

    required property var page
    required property real ratio
    property string layout
    readonly property real bezel: App.state.options?.["layout.bezel_mm"] ?? 0
    readonly property var mine: Object.keys(App.state.outputs ?? {}).filter(name => App.state.outputs[name].mm).map(name => ({
                "id": name,
                "mm": App.state.outputs[name].mm
            }))
    property var shown: ({
            "parts": []
        })

    function load() {
        if (layout === "") {
            shown = {
                "parts": []
            };
            return;
        }
        const query = {
            "spread": {
                "layout": layout,
                "bezel": bezel,
                "monitors": mine
            }
        };
        shown = JSON.parse(page.editor.rig("", JSON.stringify(query)) || "null") ?? {
            "parts": []
        };
    }

    visible: layout !== ""
    onLayoutChanged: load()
    onBezelChanged: load()
    onMineChanged: load()
    onShownChanged: shade.requestPaint()
    onRatioChanged: shade.requestPaint()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            span.load();
        }

        target: span.page.editor
    }

    Canvas {
        id: shade

        anchors.fill: parent
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
        onPaint: {
            const g = getContext("2d");
            g.reset();
            g.fillStyle = Qt.rgba(0, 0, 0, 0.65);
            g.fillRect(0, 0, width, height);
            for (const part of span.shown.parts ?? []) {
                const r = part.rect.map(v => v * span.ratio);
                g.clearRect(r[0], r[1], r[2], r[3]);
            }
        }
    }

    Repeater {
        model: span.shown.parts ?? []

        delegate: Rectangle {
            id: frame

            required property var modelData

            x: modelData.rect[0] * span.ratio
            y: modelData.rect[1] * span.ratio
            width: modelData.rect[2] * span.ratio
            height: modelData.rect[3] * span.ratio
            color: "transparent"
            border.color: Kirigami.Theme.highlightColor
            border.width: 2

            QQC2.Label {
                anchors.left: parent.left
                anchors.top: parent.top
                anchors.margins: Kirigami.Units.smallSpacing
                text: frame.modelData.id
                textFormat: Text.PlainText
                color: "white"
                style: Text.Outline
                font: Kirigami.Theme.smallFont
            }
        }
    }

    QQC2.Label {
        anchors.centerIn: parent
        width: parent.width * 0.8
        visible: !!span.shown.error || (span.layout === "mine" && span.mine.length === 0)
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        color: "white"
        style: Text.Outline
        text: span.mine.length === 0 && span.layout === "mine" ? App.tr("The desktop has not told where its screens are yet.") : App.tr("These screens cannot be shown: %1", span.shown.error ?? "")
    }
}
