import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: shapes

    required property var dialog

    signal picked

    QQC2.ComboBox {
        Layout.fillWidth: true
        textRole: "text"
        valueRole: "value"
        currentIndex: ["flat", "panorama", "faces"].indexOf(shapes.dialog.shape)
        model: [
            {
                "text": App.tr("A flat picture"),
                "value": "flat"
            },
            {
                "text": App.tr("A cube map made from this panorama"),
                "value": "panorama"
            },
            {
                "text": App.tr("A cube map made from six square pictures"),
                "value": "faces"
            }
        ]
        onActivated: {
            shapes.dialog.shape = currentValue;
            shapes.picked();
        }
    }

    GridLayout {
        Layout.fillWidth: true
        columns: 2
        visible: shapes.dialog.shape === "faces"

        Repeater {
            model: ["+X", "-X", "+Y", "-Y", "+Z", "-Z"]

            delegate: RowLayout {
                id: face

                required property string modelData
                required property int index

                Layout.fillWidth: true

                QQC2.Label {
                    text: App.tr("Face %1", face.modelData)
                }

                QQC2.ComboBox {
                    Layout.fillWidth: true
                    model: shapes.dialog.pictures.map(a => a.id)
                    currentIndex: model.indexOf(shapes.dialog.faces[face.index])
                    onActivated: {
                        const list = shapes.dialog.faces.slice();
                        list[face.index] = currentText;
                        shapes.dialog.faces = list;
                        shapes.picked();
                    }
                }
            }
        }
    }
}
