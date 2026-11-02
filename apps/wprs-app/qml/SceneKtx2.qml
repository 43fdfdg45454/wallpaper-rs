import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    required property var page
    property string asset: ""
    property string pointer: ""
    property string codec: "uastc"
    property var settings: ({
            "quality": 80
        })
    readonly property var trial: JSON.parse(packer.trial || "null")
    readonly property var classes: ({
            "bc7": App.tr("Desktop graphics cards (BC7)"),
            "astc": App.tr("Phones (ASTC)"),
            "etc2": App.tr("Older phones (ETC2)"),
            "rgba": App.tr("Any other card (uncompressed)")
        })

    function show(id, at) {
        asset = id;
        pointer = at;
        weigh();
        open();
    }

    function weigh() {
        packer.weigh(page.editor.folder, JSON.stringify({
            "asset": asset,
            "entry": JSON.parse(page.editor.value(pointer) || "{}"),
            "codec": codec,
            "quality": Math.round(settings.quality)
        }));
    }

    title: App.tr("Convert %1 to KTX2", asset)
    preferredWidth: Kirigami.Units.gridUnit * 40
    standardButtons: Kirigami.Dialog.NoButton
    customFooterActions: [
        Kirigami.Action {
            text: App.tr("Convert")
            icon.name: "document-save-as"
            enabled: !packer.busy && dialog.trial !== null && !dialog.trial.error
            onTriggered: {
                dialog.page.run("asset", {
                    "asset": dialog.asset,
                    "ktx2": true,
                    "codec": dialog.codec,
                    "quality": Math.round(dialog.settings.quality)
                });
                dialog.close();
            }
        }
    ]

    Packer {
        id: packer
    }

    Timer {
        id: later

        interval: 400
        onTriggered: dialog.weigh()
    }

    ColumnLayout {
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: App.tr("A KTX2 texture stays compressed on the graphics card: it takes about a quarter of the memory of a picture and loads faster, most of all on phones. Each card gets the format it reads best; the crops below show what each one draws.")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            textRole: "text"
            valueRole: "value"
            model: [
                {
                    "text": App.tr("Sharper (UASTC)"),
                    "value": "uastc"
                },
                {
                    "text": App.tr("Smaller file (ETC1S)"),
                    "value": "etc1s"
                }
            ]
            onActivated: {
                dialog.codec = currentValue;
                later.restart();
            }
        }

        SceneSliders {
            Layout.fillWidth: true
            value: dialog.settings
            rows: [["quality", App.tr("Quality"), 1, 100, 1]]
            onEdited: (key, value) => {
                dialog.settings = {
                    "quality": value
                };
                later.restart();
            }
        }

        QQC2.BusyIndicator {
            Layout.alignment: Qt.AlignHCenter
            running: packer.busy
            visible: packer.busy
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            visible: dialog.trial !== null
            text: dialog.trial?.error ?? App.tr("File: %1 now, %2 as KTX2, with %3 levels of detail.", App.bytes(dialog.trial?.before ?? 0), App.bytes(dialog.trial?.after ?? 0), dialog.trial?.levels ?? 0)
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            visible: dialog.trial?.resized ?? false
            text: App.tr("Widened from %1 × %2 to %3 × %4 pixels so that every card keeps it compressed.", dialog.trial?.source?.[0] ?? 0, dialog.trial?.source?.[1] ?? 0, dialog.trial?.size?.[0] ?? 0, dialog.trial?.size?.[1] ?? 0)
        }

        GridLayout {
            Layout.fillWidth: true
            columns: 3
            visible: !(dialog.trial?.error)

            SceneKtx2Look {
                source: dialog.trial?.original ?? ""
                label: App.tr("The picture now")
                detail: App.tr("%1 on the card", App.bytes(dialog.trial?.classes?.[3]?.gpu ?? 0))
            }

            Repeater {
                model: dialog.trial?.classes ?? []

                delegate: SceneKtx2Look {
                    required property var modelData

                    source: modelData.look ?? ""
                    label: dialog.classes[modelData.target] ?? modelData.target
                    detail: App.tr("%1 on the card", App.bytes(modelData.gpu))
                }
            }
        }
    }
}
