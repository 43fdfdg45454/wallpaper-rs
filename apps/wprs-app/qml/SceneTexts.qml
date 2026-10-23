import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: texts

    required property var host
    readonly property var field: host.field
    readonly property bool multiline: field.widget === "markup"
    readonly property bool last: field.key === "title" && field.options.length < 2

    function put(language, text) {
        host.page.run("translate", {
                "pointer": field.pointer,
                "language": language,
                "text": text
            });
    }

    Repeater {
        model: texts.field.options

        delegate: RowLayout {
            id: row

            required property var modelData
            readonly property string text: String(modelData.text ?? "")

            Layout.fillWidth: true

            QQC2.Label {
                visible: row.modelData.language !== ""
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: Kirigami.Units.smallSpacing
                text: row.modelData.language
            }

            QQC2.TextField {
                Layout.fillWidth: true
                visible: !texts.multiline
                text: row.text
                Accessible.name: SceneWords.label(texts.field.key) + " " + row.modelData.language
                onEditingFinished: {
                    if (text !== row.text)
                        texts.put(row.modelData.language, text);
                }
            }

            QQC2.TextArea {
                Layout.fillWidth: true
                visible: texts.multiline
                text: row.text
                wrapMode: TextEdit.Wrap
                Accessible.name: SceneWords.label(texts.field.key) + " " + row.modelData.language
                onEditingFinished: {
                    if (text !== row.text)
                        texts.put(row.modelData.language, text);
                }
            }

            QQC2.ToolButton {
                visible: row.modelData.language !== "" && !texts.last
                Layout.alignment: Qt.AlignTop
                icon.name: "edit-delete"
                text: App.tr("Remove this language")
                display: QQC2.AbstractButton.IconOnly
                QQC2.ToolTip.text: text
                QQC2.ToolTip.visible: hovered
                onClicked: texts.put(row.modelData.language, null)
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true
        visible: texts.host.page.view.v2 !== false

        QQC2.TextField {
            id: tag

            Layout.fillWidth: true
            placeholderText: App.tr("Language, such as es or pt-BR")
            onAccepted: add.clicked()
        }

        QQC2.Button {
            id: add

            icon.name: "list-add"
            text: App.tr("Translate")
            enabled: /^[A-Za-z]{2,3}(-[A-Za-z0-9]{1,8}){0,3}$/.test(tag.text)
            onClicked: {
                texts.host.page.run("translate", {
                        "pointer": texts.field.pointer,
                        "language": tag.text,
                        "add": true
                    });
                tag.clear();
            }
        }
    }
}
