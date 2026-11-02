import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

RowLayout {
    id: votes

    required property string packageId
    property var counts: null
    property bool shown
    property string mine: "none"

    signal voted(var found)

    function cast(vote) {
        const wanted = votes.mine === vote ? "none" : vote;
        App.act("Vote", [votes.packageId, wanted], "Sync1", found => {
            if (!votes)
                return;
            votes.mine = found.mine ?? "none";
            votes.voted({
                "up": found.up,
                "down": found.down
            });
        });
    }

    QQC2.ToolButton {
        icon.name: "thumb-up"
        text: votes.shown && votes.counts ? String(votes.counts.up) : App.tr("Vote up")
        checkable: true
        checked: votes.mine === "up"
        onClicked: votes.cast("up")
    }

    QQC2.ToolButton {
        icon.name: "thumb-down"
        text: votes.shown && votes.counts ? String(votes.counts.down) : App.tr("Vote down")
        checkable: true
        checked: votes.mine === "down"
        onClicked: votes.cast("down")
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Votes go to your server with your account; it keeps only the counts and your own vote.")
    }
}
