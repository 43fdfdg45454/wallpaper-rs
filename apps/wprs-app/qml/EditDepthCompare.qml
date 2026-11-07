import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: compare

    required property var page
    readonly property var relief: page.relief
    readonly property var survey: JSON.parse(relief.survey || "{}")
    readonly property int count: (survey.cuts?.length ?? 0) + 1
    readonly property var names: [App.tr("None")].concat(Array.from({
            "length": count
        }, (v, i) => i === 0 ? App.tr("Layer %1, the farthest", i + 1) : i === count - 1 ? App.tr("Layer %1, the nearest", i + 1) : App.tr("Layer %1", i + 1)))

    function show() {
        if (page.layerPick < 0)
            return relief.clear();
        if (page.splitOn)
            relief.compare(page.editor.json, page.layerPick);
        else
            relief.view(page.editor.json, page.layerPick, page.bandOn);
    }

    Connections {
        target: compare.relief

        function onBuiltChanged() {
            compare.show();
        }
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: compare.names
        currentIndex: Math.min(compare.page.layerPick, compare.count - 1) + 1
        onActivated: index => {
            compare.page.layerPick = index - 1;
            compare.show();
        }
    }

    QQC2.CheckBox {
        text: App.tr("Highlight the band that moving can uncover")
        enabled: compare.page.layerPick >= 0 && !compare.page.splitOn
        checked: compare.page.bandOn
        onToggled: {
            compare.page.bandOn = checked;
            compare.show();
        }
    }

    QQC2.Switch {
        text: App.tr("Compare LaMa with the simple fill")
        enabled: compare.page.layerPick >= 0 && compare.survey.lama === true
        checked: compare.page.splitOn
        onToggled: {
            compare.page.splitOn = checked;
            compare.show();
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: compare.survey.lama === false
        text: App.tr("LaMa is not installed, so there is nothing to compare. Download it in the settings.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }

    EditSlider {
        visible: compare.page.splitOn
        text: App.tr("Where LaMa ends")
        from: 0.02
        to: 0.98
        value: compare.page.splitAt
        onMoved: value => compare.page.splitAt = value
    }

    RowLayout {
        visible: compare.relief.busy

        QQC2.ProgressBar {
            Layout.fillWidth: true
            value: Math.max(0, compare.relief.progress)
            indeterminate: compare.relief.progress < 0
        }

        QQC2.ToolButton {
            icon.name: "process-stop"
            text: App.tr("Cancel")
            onClicked: compare.relief.cancel()
        }
    }

    QQC2.Label {
        visible: compare.relief.busy
        text: compare.relief.stage === "building" ? App.tr("Building the layers…") : App.tr("Preparing the view…")
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        type: Kirigami.MessageType.Error
        text: compare.relief.error
        visible: text !== ""
    }
}
