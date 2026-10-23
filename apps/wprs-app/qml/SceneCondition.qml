import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: condition

    required property var host
    readonly property var value: host.field.value
    readonly property string shown: typeof value === "string" ? value : value && Object.keys(value).length ? JSON.stringify(value) : ""
    property string problem: ""

    QQC2.TextField {
        Layout.fillWidth: true
        text: condition.shown
        placeholderText: App.tr("Always")
        onTextEdited: condition.problem = condition.host.page.editor.condition(condition.host.field.pointer, text)
        onEditingFinished: {
            if (text === condition.shown || condition.problem !== "")
                return;
            condition.host.page.run("condition", {
                    "id": condition.host.field.pointer.split("/")[3],
                    "enabled": condition.host.field.key === "enabled_if",
                    "text": text
                });
        }

        QQC2.ToolTip.text: App.tr("Ids of other properties, numbers, 'texts', true and false with == != < <= > >= && || ! and parentheses, such as mood == 2 || speed > 1.5")
        QQC2.ToolTip.visible: hovered
        QQC2.ToolTip.delay: Kirigami.Units.toolTipDelay
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: condition.problem !== ""
        text: condition.problem
        color: Kirigami.Theme.negativeTextColor
        wrapMode: Text.Wrap
    }
}
