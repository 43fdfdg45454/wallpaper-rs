import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: installer

    function install(spec, stale, downgrade) {
        const flags = [stale === true, downgrade === true];
        App.act("Install", [spec].concat(flags), "Repos1", null, error => {
            if (!installer)
                return;
            if (error.name === App.stale)
                installer.ask(staleness, [spec, true, flags[1]]);
            else if (error.name === App.invalidArgs && error.message.includes("downgrade"))
                installer.ask(older, [spec, flags[0], true]);
        });
    }

    function ask(dialog, retry) {
        dialog.retries = dialog.retries.concat([retry]);
        dialog.open();
    }

    function retry(dialog) {
        const retries = dialog.retries;
        dialog.retries = [];
        retries.forEach(r => installer.install(r[0], r[1], r[2]));
    }

    Kirigami.PromptDialog {
        id: staleness

        property var retries: []

        title: App.tr("The repository is out of date")
        subtitle: App.tr("Its signed index expired, so it may miss recent fixes or revocations. Install anyway?")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: installer.retry(staleness)
        onRejected: retries = []
    }

    Kirigami.PromptDialog {
        id: older

        property var retries: []

        title: App.tr("Install an older version?")
        subtitle: App.tr("A newer version of this package is installed. Install the older one too?")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: installer.retry(older)
        onRejected: retries = []
    }
}
