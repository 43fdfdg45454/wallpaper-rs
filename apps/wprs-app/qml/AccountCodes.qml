import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: codes

    required property var account
    property var device: ({})
    property int lookup: 0
    property int comparing: 0
    readonly property var found: App.jobs.find(j => j.id === codes.lookup) ?? null
    readonly property var compared: App.jobs.find(j => j.id === codes.comparing) ?? null
    readonly property var pairing: codes.account?.pairing ?? null
    readonly property string shown: codes.pairing?.sharing && codes.pairing.device === codes.device.id ? (codes.pairing.code ?? "") : ""
    readonly property string other: codes.device.label ?? codes.device.name ?? ""
    readonly property alias approver: approver

    function share(target) {
        codes.device = target;
        digits.text = "";
        App.query("ShareKey", [target.id], "Sync1", reply => codes.comparing = reply[0]);
        sharer.open();
    }

    Kirigami.Dialog {
        id: approver

        title: App.tr("Approve a device")
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: App.tr("Look up")
                icon.name: "search"
                visible: codes.lookup === 0
                onTriggered: App.query("PreviewCode", [approval.text.trim()], "Sync1", reply => codes.lookup = reply[0])
            },
            Kirigami.Action {
                text: App.tr("This is my device: approve")
                icon.name: "dialog-ok-apply"
                visible: codes.found?.state === "done"
                onTriggered: {
                    App.act("Approve", [approval.text.trim()], "Sync1");
                    approver.close();
                }
            }
        ]
        onClosed: {
            codes.lookup = 0;
            approval.text = "";
        }

        ColumnLayout {
            QQC2.Label {
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 24
                wrapMode: Text.Wrap
                text: App.tr("Approve only a code that your own new device shows on its screen. Someone who sends you a code is trying to get into your account.")
            }

            QQC2.TextField {
                id: approval

                readOnly: codes.lookup !== 0
                placeholderText: "BCDF-GHJK"
            }

            QQC2.Label {
                Layout.fillWidth: true
                visible: codes.found?.state === "done"
                wrapMode: Text.Wrap
                text: App.tr("A device on %1 running %2 asked for this code at %3.").arg(codes.found?.output?.platform ?? "").arg(codes.found?.output?.client || App.tr("unnamed software")).arg(App.time(Math.floor((codes.found?.output?.requested_at ?? 0) / 1000)))
            }

            QQC2.Label {
                Layout.fillWidth: true
                visible: codes.found?.state === "failed"
                wrapMode: Text.Wrap
                color: Kirigami.Theme.negativeTextColor
                text: codes.found?.error?.message ?? ""
            }
        }
    }

    Kirigami.Dialog {
        id: sharer

        title: App.tr("Share the sync key with %1").arg(codes.other)
        standardButtons: Kirigami.Dialog.Cancel
        customFooterActions: [
            Kirigami.Action {
                text: App.tr("Send the key")
                icon.name: "document-send"
                enabled: codes.shown !== "" && digits.text.trim() !== ""
                onTriggered: {
                    App.act("ConfirmShare", [codes.device.id, digits.text], "Sync1");
                    sharer.close();
                }
            }
        ]
        onRejected: {
            if (codes.compared && !codes.compared.finished_at)
                App.act("CancelJob", [codes.comparing]);
        }

        ColumnLayout {
            QQC2.Label {
                Layout.fillWidth: true
                Layout.preferredWidth: Kirigami.Units.gridUnit * 24
                wrapMode: Text.Wrap
                text: codes.shown ? App.tr("This computer shows %1. Type the code that %2 shows: the key leaves only when both match.").arg(codes.shown).arg(codes.other) : codes.compared?.state === "failed" ? codes.compared.error.message : App.tr("Waiting for %1: choose this computer there to get the key.").arg(codes.other)
            }

            QQC2.TextField {
                id: digits

                visible: codes.shown !== ""
                placeholderText: App.tr("The 6 digits shown on that device")
            }
        }
    }
}
