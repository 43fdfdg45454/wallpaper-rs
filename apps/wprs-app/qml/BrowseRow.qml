import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

RowLayout {
    id: row

    required property var modelData
    required property var catalog
    readonly property bool veiled: modelData.content === "mature" && !modelData.revealed && !row.catalog.shown[modelData.repo_id + "/" + modelData.id] && !App.unveiled
    readonly property bool wants: (pointer.hovered || version.activeFocus || install.activeFocus) && !veiled && !row.catalog.battery && modelData.photosensitive !== true

    onWantsChanged: {
        if (wants)
            row.catalog.animate(modelData);
    }

    Image {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 8
        Layout.preferredHeight: Kirigami.Units.gridUnit * 5
        source: App.url(row.catalog.previews[row.catalog.spec(row.modelData)] ?? "")
        asynchronous: true
        fillMode: Image.PreserveAspectCrop
        sourceSize: row.veiled ? Qt.size(12, 12) : undefined

        HoverHandler {
            id: pointer
        }

        AnimatedImage {
            anchors.fill: parent
            visible: row.wants && status === AnimatedImage.Ready
            source: row.wants ? App.url(row.catalog.moving[row.catalog.spec(row.modelData)] ?? "") : ""
            playing: visible && row.Window.visibility !== Window.Hidden
            cache: false
            fillMode: Image.PreserveAspectCrop
        }

        TapHandler {
            enabled: row.veiled
            onTapped: {
                const key = row.modelData.repo_id + "/" + row.modelData.id;
                row.catalog.shown = Object.assign({}, row.catalog.shown, {
                    [key]: true
                });
                App.act("Reveal", [key], "Repos1");
            }
        }
    }

    ColumnLayout {
        Layout.fillWidth: true

        Kirigami.Heading {
            level: 4
            text: (row.modelData.featured ? App.tr("Featured") + " · " : "") + row.modelData.title + " · " + Labels.value(row.modelData.kind)
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            textFormat: Text.PlainText
            text: [row.modelData.content ? App.plain(row.modelData.description) : row.modelData.description, row.modelData.authors.join(", ") + " · " + row.modelData.license + " · " + App.bytes(row.modelData.size), row.modelData.id + " · " + row.modelData.repo_name, row.modelData.installed.length > 0 ? App.tr("Installed: %1", row.modelData.installed.join(", ")) : "", row.modelData.update && row.modelData.changes ? App.tr("What's new: %1", App.plain(row.modelData.changes)) : "", row.veiled ? App.tr("Mature content: click the picture to show it") : ""].filter(t => t).join("\n")
        }

        QQC2.Label {
            Layout.fillWidth: true
            visible: row.modelData.photosensitive === true
            wrapMode: Text.Wrap
            color: Kirigami.Theme.negativeTextColor
            text: App.tr("Flashes or shows fast patterns that can trigger seizures in people with photosensitive epilepsy.")
        }

        BrowseVotes {
            counts: row.catalog.votes[row.modelData.id] ?? null
            shown: row.catalog.counted
            onVoted: found => row.catalog.votes = Object.assign({}, row.catalog.votes, Object.fromEntries([[row.modelData.id, found]]))
            packageId: row.modelData.id
        }

        PrivacyNotice {
            Layout.fillWidth: true
            exposes: row.modelData.kind === "web" ? ["zone", "language", "gpu"] : []
        }
    }

    QQC2.ComboBox {
        id: version

        model: row.modelData.versions
        currentIndex: count - 1
    }

    QQC2.Button {
        id: install

        text: row.modelData.update ? App.tr("Update") : App.tr("Install")
        icon.name: "download"
        onClicked: row.catalog.installer.install(row.modelData.repo_id + "/" + row.modelData.id + "@" + version.currentText)
    }
}
