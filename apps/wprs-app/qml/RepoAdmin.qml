import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

RowLayout {
    id: admin

    required property var stall
    required property var backend

    function revokePackage(id, version) {
        revoking.target = ["package", id, version];
        revoking.ask(version ? App.tr("Revoke %1 %2: clients no longer offer or install it. Publish next.", id, version) : App.tr("Revoke every version of %1: clients no longer offer or install them. Publish next.", id), true);
    }

    QQC2.Button {
        icon.name: "folder-new"
        text: App.tr("New repository…")
        enabled: !admin.stall.busy
        onClicked: charter.start(null)
    }

    QQC2.Button {
        icon.name: "document-encrypted"
        text: App.tr("Root…")
        enabled: admin.stall.shown !== null && !admin.stall.busy
        onClicked: rooting.open()
    }

    QQC2.Button {
        icon.name: "view-certificate"
        text: App.tr("Author keys…")
        enabled: admin.stall.shown !== null && !admin.stall.busy
        onClicked: keying.open()
    }

    RepoCharter {
        id: charter

        onMade: (folder, text) => {
            if (fresh) {
                admin.stall.folder = folder;
                admin.backend.create(folder, text);
            } else {
                admin.backend.rotate(admin.stall.folder, text);
            }
        }
    }

    RepoRoot {
        id: rooting

        root: admin.stall.shown?.root ?? null
        onSign: (key, password) => admin.backend.signRoot(admin.stall.folder, key, password)
        onRotate: {
            rooting.close();
            charter.start(admin.stall.shown);
        }
    }

    RepoKeys {
        id: keying

        shown: admin.stall.shown
        onRevoke: fingerprint => {
            revoking.target = ["key", fingerprint, ""];
            revoking.ask(App.tr("Revoke the author key %1: every signature it made stops counting. Publish next.", fingerprint), false);
        }
    }

    RepoRevoke {
        id: revoking

        property var target: []

        onRevoke: (reason, message) => {
            if (target[0] === "key")
                admin.backend.revokeKey(admin.stall.folder, target[1], reason);
            else
                admin.backend.revoke(admin.stall.folder, target[1], target[2], reason, message);
        }
    }
}
