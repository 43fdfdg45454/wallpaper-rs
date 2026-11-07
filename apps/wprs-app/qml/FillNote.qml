import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: note

    readonly property var lama: JSON.parse(maps.assets || "[]").find(asset => asset.inpaint)
    readonly property bool wanted: (App.state.options ?? {})["parallax.fill"] === "lama"
    readonly property var kept: JSON.parse(maps.layers || "{}")
    readonly property var building: App.jobs.filter(job => job.kind === "layers" && (job.state === "queued" || job.state === "running"))
    property bool rebuilding

    DepthMaps {
        id: maps

        Component.onCompleted: open("")
        onBusyChanged: {
            if (!busy && note.rebuilding) {
                note.rebuilding = false;
                App.act("Reload", []);
            }
        }
    }

    ColumnLayout {
        visible: note.wanted && note.lama !== undefined && !note.lama.installed

        QQC2.Label {
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 28
            text: note.lama ? App.tr("LaMa is not installed, so pictures use the simple fill. It is downloaded once from %1 (%2, license %3), checked against its SHA-256 and runs on this computer; computing the layers of a picture takes a few minutes.", note.lama.hosts, note.lama.size, note.lama.license) : ""
            wrapMode: Text.Wrap
        }

        RowLayout {
            QQC2.Button {
                text: App.tr("Download LaMa")
                icon.name: "download"
                enabled: maps.allowed && !maps.busy
                onClicked: maps.fetchLama()
            }

            QQC2.Label {
                visible: !maps.allowed
                text: App.tr("Downloads are turned off in the settings.")
            }
        }
    }

    RowLayout {
        visible: note.lama !== undefined && note.lama.installed

        QQC2.Button {
            text: App.tr("Check LaMa")
            icon.name: "dialog-ok-apply"
            enabled: !maps.busy
            onClicked: maps.verifyLama()
        }

        QQC2.Button {
            text: App.tr("Remove LaMa")
            icon.name: "edit-delete"
            enabled: !maps.busy
            onClicked: maps.removeLama()
        }

        QQC2.Label {
            visible: maps.note === "verified"
            text: App.tr("LaMa is whole")
        }
    }

    RowLayout {
        QQC2.Button {
            text: App.tr("Rebuild the layers")
            icon.name: "view-refresh"
            enabled: !maps.busy && note.building.length === 0 && (note.kept.sets ?? 0) > 0
            onClicked: {
                note.rebuilding = true;
                maps.rebuildLayers();
            }
        }

        QQC2.Label {
            text: note.building.length > 0 ? App.tr("Building the layers: %1 %", Math.round(note.building[0].total > 0 ? 100 * note.building[0].done / note.building[0].total : 0)) : (note.kept.sets ?? 0) > 0 ? App.tr("%1 sets of layers are kept (%2 MB)", note.kept.sets, Math.round(note.kept.bytes / 1e6)) : App.tr("No layers are kept yet")
        }
    }

    RowLayout {
        visible: maps.busy

        QQC2.ProgressBar {
            from: 0
            to: 1
            indeterminate: maps.progress < 0
            value: maps.progress
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: maps.error !== ""
        text: maps.error
        wrapMode: Text.Wrap
    }
}
