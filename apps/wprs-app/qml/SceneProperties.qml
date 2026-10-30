import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: panel

    required property var page
    readonly property var properties: page.view.properties ?? []
    readonly property var presets: page.view.presets ?? []
    property alias list: list
    property alias tools: tools

    function add(kind) {
        let n = 1;
        while (properties.some(p => p.id.toLowerCase() === kind + "_" + n))
            n++;
        page.run("add_property", {
            "kind": kind,
            "id": kind + "_" + n
        });
        if (properties.some(p => p.id === kind + "_" + n))
            page.editor.select("/manifest/properties/" + kind + "_" + n);
    }

    function addPreset() {
        const at = presets.length;
        page.run("add_preset");
        if (presets.length > at)
            page.editor.select("/manifest/presets/" + at);
    }

    SceneSettingMenu {
        id: tools

        page: panel.page
    }

    ColumnLayout {
        anchors.fill: parent

        SceneSettingsHead {
            Layout.fillWidth: true
            page: panel.page
        }

        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                id: list

                clip: true
                model: panel.properties

                delegate: SceneSetting {
                    page: panel.page
                    tools: panel.tools
                }

                footer: ColumnLayout {
                    width: list.width
                    spacing: 0

                    QQC2.ToolButton {
                        text: App.tr("Add property")
                        icon.name: "list-add"
                        onClicked: kinds.popup()

                        QQC2.Menu {
                            id: kinds

                            Repeater {
                                model: panel.page.view.property_kinds ?? []

                                delegate: QQC2.MenuItem {
                                    required property string modelData

                                    text: SceneWords.label(modelData)
                                    onTriggered: panel.add(modelData)
                                }
                            }
                        }
                    }

                    Kirigami.Heading {
                        Layout.fillWidth: true
                        level: 4
                        text: App.tr("Presets")
                    }

                    Repeater {
                        model: panel.presets

                        delegate: QQC2.ItemDelegate {
                            id: preset

                            required property var modelData

                            Layout.fillWidth: true
                            highlighted: panel.page.editor.selected === modelData.pointer
                            icon.name: "bookmarks"
                            text: modelData.label
                            onClicked: panel.page.editor.select(modelData.pointer)

                            QQC2.ToolButton {
                                anchors.right: parent.right
                                icon.name: "edit-delete"
                                text: App.tr("Remove")
                                display: QQC2.AbstractButton.IconOnly
                                QQC2.ToolTip.text: text
                                QQC2.ToolTip.visible: hovered
                                onClicked: {
                                    panel.page.editor.select("/manifest");
                                    panel.page.run("remove", {
                                        "pointer": preset.modelData.pointer
                                    });
                                }
                            }
                        }
                    }

                    QQC2.ToolButton {
                        text: App.tr("Add preset")
                        icon.name: "list-add"
                        onClicked: panel.addPreset()
                    }
                }
            }
        }
    }
}
