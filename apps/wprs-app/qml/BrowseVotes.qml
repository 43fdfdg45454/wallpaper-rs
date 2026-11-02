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
    readonly property string mine: votes.counts?.mine ?? "none"

    signal voted(var found)

    function cast(vote) {
        const wanted = votes.mine === vote ? "none" : vote;
        App.act("Vote", [votes.packageId, wanted], "Sync1", found => {
            if (!votes)
                return;
            votes.voted({
                "up": found.up,
                "down": found.down,
                "mine": found.mine ?? "none"
            });
        });
    }

    QQC2.ToolButton {
        icon.name: "thumb-up"
        text: votes.shown && votes.counts ? String(votes.counts.up) : App.tr("Vote up")
        checked: votes.mine === "up"
        onClicked: votes.cast("up")
    }

    QQC2.ToolButton {
        icon.name: "thumb-down"
        text: votes.shown && votes.counts ? String(votes.counts.down) : App.tr("Vote down")
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
