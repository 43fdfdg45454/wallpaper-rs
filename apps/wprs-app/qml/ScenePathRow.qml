import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: row

    required property var flight
    required property var modelData
    readonly property string id_: modelData.id

    RowLayout {
        QQC2.Label {
            Layout.fillWidth: true
            text: App.tr("%1, %2 s", row.id_, row.modelData.period)
            font.bold: true
        }

        QQC2.CheckBox {
            text: App.tr("Loop")
            checked: row.modelData.loop === true
            onToggled: row.flight.path("set", {
                "path": row.id_,
                "loop": checked
            })
        }

        QQC2.ComboBox {
            model: row.flight.eases.map(e => SceneWords.label(e))
            currentIndex: Math.max(0, row.flight.eases.indexOf(row.modelData.ease ?? "linear"))
            onActivated: index => row.flight.path("set", {
                    "path": row.id_,
                    "ease": row.flight.eases[index]
                })
        }

        QQC2.ToolButton {
            icon.name: "edit-delete"
            onClicked: row.flight.path("remove", {
                "path": row.id_
            })
        }
    }

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        SceneRepeater {
            id: listed

            rows: row.modelData.keys

            delegate: QQC2.Button {
                required property int index
                readonly property var modelData: listed.rows[index]

                text: App.tr("%1 s", modelData.t)
                icon.name: "edit-delete-remove"
                QQC2.ToolTip.visible: hovered
                QQC2.ToolTip.text: App.tr("Remove this key")
                onClicked: row.flight.path("key", {
                    "path": row.id_,
                    "time": modelData.t,
                    "remove": true
                })
            }
        }

        QQC2.Button {
            icon.name: "list-add"
            text: App.tr("Key the view at %1 s", row.flight.page.editor.time.toFixed(2))
            onClicked: row.flight.path("key", {
                "path": row.id_,
                "time": Math.round(row.flight.page.editor.time * 100) / 100,
                "from_view": true
            })
        }
    }
}
