import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

RowLayout {
    id: templates

    required property var page
    readonly property var names: Object.keys(JSON.parse(page.view ? page.editor.value("/scene/templates") || "null" : "null") ?? {})
    readonly property string alone: JSON.parse(page.editor.look || "{}").template ?? ""

    function show(name) {
        const look = JSON.parse(page.editor.look || "{}");
        if (name)
            look.template = name;
        else
            delete look.template;
        page.editor.look = Object.keys(look).length > 0 ? JSON.stringify(look) : "";
        page.editor.seek(page.clock, page.editor.playing);
        if (name)
            page.editor.select("/scene/templates/" + name);
    }

    visible: names.length > 0

    QQC2.ComboBox {
        model: [App.tr("The scene")].concat(templates.names.map(n => App.tr("Template %1 alone", n)))
        currentIndex: templates.names.indexOf(templates.alone) + 1
        onActivated: index => templates.show(index > 0 ? templates.names[index - 1] : "")
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.text: App.tr("Show one template alone on the canvas, as a copy of it draws, and edit its layers with the layers panel and the canvas tools")
    }
}
