import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

QQC2.ToolBar {
    id: bar

    readonly property bool paused: App.state.paused === true
    readonly property bool muted: App.state.muted === true
    readonly property var active: App.jobs.filter(job => job.state === "queued" || job.state === "running")

    RowLayout {
        anchors.fill: parent

        QQC2.ToolButton {
            icon.name: bar.paused ? "media-playback-start" : "media-playback-pause"
            text: bar.paused ? App.tr("Resume") : App.tr("Pause")
            onClicked: App.act("SetPaused", [!bar.paused])
        }

        QQC2.ToolButton {
            icon.name: bar.muted ? "audio-volume-muted" : "audio-volume-high"
            text: bar.muted ? App.tr("Unmute") : App.tr("Mute")
            display: QQC2.AbstractButton.IconOnly
            QQC2.ToolTip.text: text
            QQC2.ToolTip.visible: hovered
            onClicked: App.act("SetMuted", [!bar.muted])
        }

        QQC2.Slider {
            id: volume

            Layout.preferredWidth: Kirigami.Units.gridUnit * 8
            from: 0
            to: 1
            value: App.state.volume ?? 0
            onMoved: debounce.restart()
        }

        Timer {
            id: debounce

            interval: 200
            onTriggered: {
                App.act("SetVolume", [volume.value]);
                App.act("SetMuted", [false]);
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            text: App.backend.lastError
            color: Kirigami.Theme.negativeTextColor
            elide: Text.ElideRight
        }

        QQC2.BusyIndicator {
            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
            visible: bar.active.length > 0
            running: visible
        }

        QQC2.ToolButton {
            icon.name: "view-process-system"
            text: bar.active.length > 0 ? App.tr("Jobs: %1").arg(bar.active.length) : App.tr("Jobs")
            onClicked: jobs.open()
        }
    }

    QQC2.Popup {
        id: jobs

        x: bar.width - width
        y: -height
        width: Kirigami.Units.gridUnit * 24
        height: Math.min(Kirigami.Units.gridUnit * 20, Math.max(list.contentHeight, empty.implicitHeight) + padding * 2)

        QQC2.Label {
            id: empty

            visible: list.count === 0
            text: App.tr("No jobs")
        }

        ListView {
            id: list

            anchors.fill: parent
            clip: true
            model: App.jobs

            delegate: RowLayout {
                id: job

                required property var modelData

                width: ListView.view.width

                ColumnLayout {
                    Layout.fillWidth: true

                    QQC2.Label {
                        Layout.fillWidth: true
                        text: [Labels.value(job.modelData.kind), job.modelData.target, Labels.value(job.modelData.state)].filter(t => t).join(" · ")
                        elide: Text.ElideMiddle
                    }

                    QQC2.ProgressBar {
                        Layout.fillWidth: true
                        visible: job.modelData.state === "running"
                        indeterminate: job.modelData.total === 0
                        value: job.modelData.total > 0 ? job.modelData.done / job.modelData.total : 0
                    }

                    QQC2.Label {
                        Layout.fillWidth: true
                        visible: !!job.modelData.error
                        text: job.modelData.error?.message || App.tr("The job failed without giving a reason")
                        color: Kirigami.Theme.negativeTextColor
                        wrapMode: Text.Wrap
                    }
                }

                QQC2.ToolButton {
                    icon.name: "process-stop"
                    visible: job.modelData.state === "queued" || job.modelData.state === "running"
                    onClicked: App.act("CancelJob", [job.modelData.id])
                    QQC2.ToolTip.text: App.tr("Cancel")
                    QQC2.ToolTip.visible: hovered
                }
            }
        }
    }
}
