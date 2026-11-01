import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: cut

    required property var page
    readonly property var pictures: (page.view.assets ?? []).filter(asset => asset.kind === "image").map(asset => asset.id)

    title: App.tr("Mesh from a picture")
    standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
    onAccepted: page.run("mesh_from_picture", {
        "image": cut.pictures[picture.currentIndex],
        "outline": outline.checked,
        "cells": cells.value
    })

    ColumnLayout {
        QQC2.ComboBox {
            id: picture

            implicitWidth: Kirigami.Units.gridUnit * 20
            model: cut.pictures
        }

        QQC2.CheckBox {
            id: outline

            text: App.tr("Follow the outline of the picture")
        }

        RowLayout {
            QQC2.Label {
                text: App.tr("Cells along the longer side")
            }

            QQC2.SpinBox {
                id: cells

                from: 2
                to: 64
                value: 16
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: App.tr("Then place its bones in the inspector and drag them on the preview.")
        }
    }
}
