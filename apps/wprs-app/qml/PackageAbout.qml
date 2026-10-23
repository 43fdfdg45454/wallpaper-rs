import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: about

    required property var info
    property string uid
    property var pkg: null
    readonly property bool styled: info.schema === "wallpaper-rs.wpak/2"
    readonly property bool asks: !!pkg && pkg.photosensitive === true && !(pkg.confirmed ?? []).includes("photosensitive")
    readonly property var works: (info.derived_from ?? []).map(w => w.title ? App.tr("%1 by %2").arg(w.title).arg(w.author) : w.author)

    function words(list) {
        return (list ?? []).map(Labels.value).join(", ");
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: about.info.photosensitive === true
        type: Kirigami.MessageType.Warning
        text: about.asks ? App.tr("This wallpaper flashes or shows fast patterns that can trigger seizures in people with photosensitive epilepsy. Playlists skip it until you allow it.") : App.tr("This wallpaper flashes or shows fast patterns that can trigger seizures in people with photosensitive epilepsy.")
        actions: [
            Kirigami.Action {
                visible: about.asks && about.uid !== ""
                text: App.tr("Allow this wallpaper…")
                icon.name: "dialog-ok-apply"
                onTriggered: allow.open()
            }
        ]
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: text !== ""
        text: about.styled ? App.markup(about.info.description) : (about.info.description ?? "")
        textFormat: about.styled ? Text.StyledText : Text.PlainText
        wrapMode: Text.Wrap
        onLinkActivated: link => opener.ask(link)
    }

    Kirigami.FormLayout {
        Layout.fillWidth: true

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Rating:")
            text: [Labels.value(about.info.content ?? "unrated"), about.words(about.info.descriptors)].filter(t => t).join(" · ")
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Origin:")
            visible: !!about.info.origin
            Layout.maximumWidth: Kirigami.Units.gridUnit * 14
            text: [Labels.value(about.info.origin)].concat(about.works).join("\n")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Authors:")
            visible: text !== ""
            text: (about.info.authors ?? []).join(", ")
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Credits:")
            visible: text !== ""
            Layout.maximumWidth: Kirigami.Units.gridUnit * 14
            text: (about.info.credits ?? []).map(c => c.paths.join(", ") + ": " + c.attribution + " (" + c.license + ")").join("\n")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Uses:")
            visible: text !== ""
            text: about.words(about.info.inputs)
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Needs:")
            visible: text !== ""
            Layout.maximumWidth: Kirigami.Units.gridUnit * 14
            text: (about.info.features ?? []).map(f => Labels.feature(f.name)).join("\n")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Languages:")
            visible: (about.info.languages ?? []).length > 1
            text: (about.info.languages ?? []).join(", ")
            textFormat: Text.PlainText
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("What's new:")
            visible: text !== ""
            Layout.maximumWidth: Kirigami.Units.gridUnit * 14
            text: App.markup(about.info.changes)
            textFormat: Text.StyledText
            wrapMode: Text.Wrap
            onLinkActivated: link => opener.ask(link)
        }
    }

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        Repeater {
            model: about.info.links ?? []

            delegate: QQC2.Button {
                required property var modelData

                text: Labels.value(modelData.kind)
                icon.name: "internet-services"
                onClicked: opener.ask(modelData.url)
            }
        }
    }

    Kirigami.PromptDialog {
        id: allow

        title: App.tr("Allow this wallpaper?")
        subtitle: App.tr("It flashes or shows fast patterns that can trigger seizures in people with photosensitive epilepsy. Allow it only if that is safe for whoever sees your screens; screens and playlists may then show it without asking again.")
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: App.act("Confirm", [[about.uid], "photosensitive"], "Library1")
    }

    Kirigami.PromptDialog {
        id: opener

        property string url

        function ask(link) {
            url = link;
            open();
        }

        title: App.tr("Open this address in the browser?")
        subtitle: url
        standardButtons: Kirigami.Dialog.Ok | Kirigami.Dialog.Cancel
        onAccepted: Qt.openUrlExternally(url)
    }
}
