import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tools

    required property var page
    property string map
    readonly property var relief: page.relief
    readonly property string key: JSON.stringify(page.edit.depth ?? {})
    readonly property string look: page.overlayOn + "/" + page.overlayColour

    function refresh() {
        if (map === "")
            return;
        if (page.overlayOn)
            relief.look(page.editor.json, page.overlayColour);
        relief.measure(page.editor.json);
    }

    onKeyChanged: {
        refresh();
        if (visible && map !== "")
            relief.sync(page.editor.json);
    }
    onMapChanged: refresh()
    onVisibleChanged: refresh()
    onLookChanged: refresh()
    Component.onCompleted: refresh()

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Depth over the picture")
    }

    EditDepthView {
        Layout.fillWidth: true
        page: tools.page
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Depth brush")
    }

    EditDepthBrush {
        Layout.fillWidth: true
        page: tools.page
        kind: "depth"
        counted: tools.relief.survey ? JSON.parse(tools.relief.survey).strokes ?? 0 : 0
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Layer cuts")
    }

    EditDepthCuts {
        Layout.fillWidth: true
        page: tools.page
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Fill brush")
    }

    EditDepthBrush {
        Layout.fillWidth: true
        page: tools.page
        kind: "fill"
        counted: tools.relief.survey ? JSON.parse(tools.relief.survey).marks ?? 0 : 0
    }
}
