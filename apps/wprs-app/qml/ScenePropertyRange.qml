import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.FormLayout {
    id: range

    required property var page
    required property string pointer
    readonly property string id: /^\/manifest\/properties\/[^\/]+$/.test(pointer) ? pointer.split("/")[3].replace(/~1/g, "/").replace(/~0/g, "~") : ""
    readonly property var prop: id ? JSON.parse(page.editor.value(pointer) || "null") : null
    readonly property var keys: [["min", App.tr("Minimum")], ["max", App.tr("Maximum")], ["step", App.tr("Step")], ["default", App.tr("Default")]]

    visible: prop?.type === "slider"

    Repeater {
        model: range.keys

        delegate: QQC2.TextField {
            required property var modelData

            visible: range.visible
            Kirigami.FormData.label: modelData[1] + ":"
            text: String(range.prop?.[modelData[0]] ?? "")
            inputMethodHints: Qt.ImhFormattedNumbersOnly
            validator: DoubleValidator {
                locale: "C"
            }
            onEditingFinished: {
                const number = Number(text);
                if (text !== "" && number !== range.prop?.[modelData[0]])
                    range.page.run("property", {
                        "id": range.id,
                        [modelData[0]]: number
                    });
            }
        }
    }
}
