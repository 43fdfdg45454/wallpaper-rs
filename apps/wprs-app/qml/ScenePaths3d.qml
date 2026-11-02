import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: flight

    required property var page
    property var info: null
    readonly property var eases: ["linear", "in_out_sine", "in_out_cubic", "in_sine", "out_sine"]
    readonly property var fades: ["", "black", "cross"]

    function load() {
        info = JSON.parse(page.editor.rig("", JSON.stringify({
            "paths3d": {
                "time": page.editor.time
            }
        })) || "null");
    }

    function path(kind, args) {
        const out = {};
        out[kind] = args;
        page.run("scene3d", {
            "path": out
        }, true);
    }

    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            flight.load();
        }

        target: flight.page.editor
    }

    Kirigami.Heading {
        level: 5
        text: App.tr("Camera paths")
    }

    Repeater {
        model: flight.info?.paths ?? []

        delegate: ScenePathRow {
            Layout.fillWidth: true
            flight: flight
        }
    }

    RowLayout {
        QQC2.TextField {
            id: name

            Layout.fillWidth: true
            placeholderText: App.tr("Name of a new path")
            validator: RegularExpressionValidator {
                regularExpression: /[a-z0-9_]{1,32}/
            }
        }

        QQC2.Button {
            icon.name: "list-add"
            text: App.tr("Add from the view")
            enabled: name.acceptableInput
            onClicked: {
                flight.path("add", {
                    "id": name.text
                });
                name.text = "";
            }
        }
    }

    RowLayout {
        visible: (flight.info?.paths ?? []).length > 0

        QQC2.ComboBox {
            model: [App.tr("In order"), App.tr("At random")]
            currentIndex: flight.info?.order === "random" ? 1 : 0
            onActivated: index => flight.path("order", {
                    "order": index === 1 ? "random" : "sequential"
                })
        }

        QQC2.ComboBox {
            model: [App.tr("Cut"), App.tr("Through black"), App.tr("Crossing")]
            currentIndex: Math.max(0, flight.fades.indexOf(flight.info?.fade?.to ?? ""))
            onActivated: index => flight.path("fade", index === 0 ? {
                    "clear": true
                } : {
                    "to": flight.fades[index]
                })
        }

        QQC2.SpinBox {
            enabled: flight.info?.fade !== null && flight.info?.fade !== undefined
            from: 0
            to: 100
            value: Math.round((flight.info?.fade?.duration ?? 1) * 10)
            textFromValue: v => App.tr("%1 s", (v / 10).toFixed(1))
            onValueModified: flight.path("fade", {
                "duration": value / 10
            })
        }
    }

    QQC2.Button {
        visible: (flight.info?.paths ?? []).length > 0
        icon.name: "media-playback-start"
        text: App.tr("Fly through the paths from the camera")
        onClicked: {
            flight.page.run("scene3d", {
                "view": {
                    "through": true
                }
            }, true);
            flight.page.editor.seek(0, true);
        }
    }
}
