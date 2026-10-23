import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.plasma.components as PlasmaComponents3
import org.kde.plasma.extras as PlasmaExtras

PlasmaExtras.Representation {
    id: panel

    required property var daemon
    readonly property var current: daemon.current
    readonly property var outputs: Object.keys(current.outputs ?? {})

    Layout.minimumWidth: Kirigami.Units.gridUnit * 16
    Layout.preferredWidth: Kirigami.Units.gridUnit * 22
    Layout.minimumHeight: Kirigami.Units.gridUnit * 10
    Layout.preferredHeight: Math.min(implicitHeight, Kirigami.Units.gridUnit * 30)
    collapseMarginsHint: true

    header: PlasmaExtras.PlasmoidHeading {
        Header {
            anchors.fill: parent
            daemon: panel.daemon
        }
    }

    PlasmaComponents3.ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth
        PlasmaComponents3.ScrollBar.horizontal.policy: PlasmaComponents3.ScrollBar.AlwaysOff

        Item {
            width: scroll.availableWidth
            implicitHeight: list.implicitHeight + 2 * list.anchors.margins

            ColumnLayout {
                id: list

                anchors.fill: parent
                anchors.margins: Kirigami.Units.largeSpacing
                spacing: Kirigami.Units.largeSpacing

                Consent {
                    Layout.fillWidth: true
                    daemon: panel.daemon
                    inputs: panel.daemon.inputs
                }

                PlasmaComponents3.Label {
                    Layout.fillWidth: true
                    visible: panel.daemon.lastError !== ""
                    text: panel.daemon.lastError
                    color: Kirigami.Theme.negativeTextColor
                    wrapMode: Text.Wrap
                }

                Kirigami.PlaceholderMessage {
                    Layout.fillWidth: true
                    visible: panel.outputs.length === 0 && panel.daemon.lastError === ""
                    icon.name: "preferences-desktop-wallpaper"
                    text: i18n("No screen shows wallpaper-rs")
                    explanation: i18n("Choose wallpaper-rs as the wallpaper type in the desktop settings.")
                }

                Repeater {
                    model: panel.outputs

                    delegate: ScreenRow {
                        required property string modelData

                        Layout.fillWidth: true
                        daemon: panel.daemon
                        name: modelData
                        info: (panel.current.outputs ?? {})[modelData] ?? {}
                    }
                }
            }
        }
    }

    footer: PlasmaExtras.PlasmoidHeading {
        position: PlasmaComponents3.ToolBar.Footer

        RowLayout {
            anchors.fill: parent
            spacing: Kirigami.Units.smallSpacing

            PlasmaComponents3.Label {
                Layout.fillWidth: true
                visible: !panel.daemon.hasApp
                text: i18n("Change the other settings with wprs-ctl")
                wrapMode: Text.Wrap
                color: Kirigami.Theme.disabledTextColor
            }

            PlasmaComponents3.Button {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                visible: panel.daemon.hasApp
                icon.name: "preferences-desktop-wallpaper"
                text: i18n("Open wallpaper-rs")
                onClicked: panel.daemon.openApp("")
            }

            PlasmaComponents3.Button {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                visible: panel.daemon.hasApp
                icon.name: "configure"
                text: i18n("Settings…")
                onClicked: panel.daemon.openApp("settings")
            }
        }
    }
}
