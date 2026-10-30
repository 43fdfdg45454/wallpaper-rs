import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Loader {
    id: meta

    required property var host
    readonly property var list: Array.isArray(host.value) ? host.value : []
    readonly property bool animated: host.field.widget === "ref:animated"

    sourceComponent: ({
            "flags": flags,
            "lines": lines,
            "computed": computed
        })[host.field.widget] ?? picture

    Component {
        id: flags

        Flow {
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: meta.host.field.options

                delegate: QQC2.CheckBox {
                    required property var modelData

                    text: (meta.host.field.key === "inputs" ? ManifestWords.inputs[modelData] : undefined) ?? SceneWords.value(modelData)
                    checked: meta.list.includes(modelData)
                    enabled: !meta.host.field.fixed.includes(modelData)
                    onToggled: meta.host.commit(checked ? meta.list.concat([modelData]) : meta.list.filter(item => item !== modelData))
                }
            }
        }
    }

    Component {
        id: lines

        Area {
            readonly property string joined: meta.list.join("\n")

            text: joined
            wrapMode: TextEdit.Wrap
            placeholderText: App.tr("One per line")
            onEditingFinished: {
                if (text !== joined)
                    meta.host.commit(text.split("\n").map(line => line.trim()).filter(line => line !== ""));
            }
        }
    }

    Component {
        id: computed

        ColumnLayout {
            QQC2.Label {
                Layout.fillWidth: true
                text: App.tr("Filled from what the package uses: %1", meta.list.map(name => Labels.feature(name)).join(", ") || App.tr("nothing"))
                wrapMode: Text.Wrap
                color: Kirigami.Theme.disabledTextColor
            }

            Repeater {
                model: meta.host.field.options

                delegate: QQC2.CheckBox {
                    required property var modelData

                    text: Labels.feature(modelData)
                    checked: meta.list.includes(modelData)
                    onToggled: meta.host.commit(checked ? meta.list.concat([modelData]) : meta.list.filter(item => item !== modelData))
                }
            }
        }
    }

    Component {
        id: picture

        RowLayout {
            QQC2.Label {
                Layout.fillWidth: true
                text: String(meta.host.value ?? "") || App.tr("No picture")
                elide: Text.ElideMiddle
            }

            QQC2.Button {
                text: App.tr("Choose…")
                icon.name: "document-open"
                onClicked: pictures.open()
            }

            FileDialog {
                id: pictures

                nameFilters: [App.tr("Pictures") + (meta.animated ? " (*.gif *.webp)" : " (*.png *.jpg *.jpeg *.webp *.gif)")]
                onAccepted: meta.host.page.run("thumbnail", {
                    "file": App.path(selectedFile),
                    "animated": meta.animated
                })
            }
        }
    }
}
