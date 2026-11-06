import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: stock

    required property var modelData
    required property var stall
    readonly property bool shown: modelData.versions.some(v => v.listed)

    function title(text) {
        return typeof text === "string" ? text : (Object.values(text ?? {})[0] ?? "");
    }

    RowLayout {
        Layout.fillWidth: true

        Kirigami.Heading {
            level: 4
            text: (stock.modelData.featured ? "★ " : "") + stock.modelData.id
        }

        QQC2.Label {
            Layout.fillWidth: true
            elide: Text.ElideRight
            opacity: 0.7
            text: stock.modelData.note ?? (stock.modelData.author ? App.tr("by %1", stock.modelData.author) : "")
        }

        QQC2.Button {
            text: stock.shown ? App.tr("Unlist") : App.tr("List again")
            enabled: stock.stall !== undefined && !stock.stall.busy
            onClicked: stock.stall.ask(stock.shown ? App.tr("Unlist %1: out of browsing and search, still installable by id.", stock.modelData.id) : App.tr("List %1 again in browsing and search.", stock.modelData.id), () => stock.stall.list(stock.modelData.id, "", !stock.shown))
        }

        QQC2.Button {
            text: stock.modelData.revoked ? App.tr("Withdraw the revocation") : App.tr("Revoke…")
            enabled: stock.stall !== undefined && !stock.stall.busy
            onClicked: {
                if (stock.modelData.revoked)
                    stock.stall.ask(App.tr("Withdraw the revocation of every version of %1. Publish next.", stock.modelData.id), () => stock.stall.unrevoke(stock.modelData.id, ""));
                else
                    stock.stall.revoke(stock.modelData.id, "");
            }
        }

        QQC2.TextField {
            id: note

            visible: !stock.modelData.featured
            placeholderText: App.tr("Note, optional")
            maximumLength: 200
        }

        QQC2.Button {
            text: stock.modelData.featured ? App.tr("Unfeature") : App.tr("Feature")
            enabled: stock.stall !== undefined && !stock.stall.busy
            onClicked: stock.stall.ask(stock.modelData.featured ? App.tr("Stop featuring %1.", stock.modelData.id) : App.tr("Feature %1, signed into the repository.", stock.modelData.id), () => stock.stall.feature(stock.modelData.id, note.text, !stock.modelData.featured))
        }
    }

    Repeater {
        model: stock.modelData.versions

        delegate: RowLayout {
            id: version

            required property var modelData

            Layout.leftMargin: Kirigami.Units.gridUnit

            QQC2.Label {
                Layout.fillWidth: true
                text: version.modelData.version + " · " + stock.title(version.modelData.title) + (version.modelData.revoked ? " · " + App.tr("revoked") : "") + (version.modelData.listed ? "" : " · " + App.tr("unlisted"))
            }

            QQC2.Button {
                flat: true
                enabled: stock.stall !== undefined && !stock.stall.busy
                text: version.modelData.listed ? App.tr("Unlist this version") : App.tr("List this version again")
                onClicked: stock.stall.ask(version.modelData.listed ? App.tr("Unlist %1 %2: out of browsing and search, still installable by id.", stock.modelData.id, version.modelData.version) : App.tr("List %1 %2 again in browsing and search.", stock.modelData.id, version.modelData.version), () => stock.stall.list(stock.modelData.id, version.modelData.version, !version.modelData.listed))
            }

            QQC2.Button {
                flat: true
                visible: !stock.modelData.revoked
                enabled: stock.stall !== undefined && !stock.stall.busy
                text: version.modelData.revoked ? App.tr("Withdraw the revocation") : App.tr("Revoke this version…")
                onClicked: {
                    if (version.modelData.revoked)
                        stock.stall.ask(App.tr("Withdraw the revocation of %1 %2. Publish next.", stock.modelData.id, version.modelData.version), () => stock.stall.unrevoke(stock.modelData.id, version.modelData.version));
                    else
                        stock.stall.revoke(stock.modelData.id, version.modelData.version);
                }
            }
        }
    }
}
