import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Card {
    id: card

    required property var provider
    readonly property var kind: (App.schema.providers ?? []).find(k => k.kind === provider.kind) ?? {}

    banner.title: provider.id + " · " + provider.kind
    actions: [
        Kirigami.Action {
            text: App.tr("Sync now")
            icon.name: "view-refresh"
            enabled: card.provider.state !== "syncing" && card.provider.state !== "needs_secret"
            onTriggered: App.act("Sync", [card.provider.id], "Providers1")
        },
        Kirigami.Action {
            text: App.tr("Set secret…")
            icon.name: "dialog-password"
            visible: !!card.kind.secret
            onTriggered: dialogs.secret.open()
        },
        Kirigami.Action {
            text: App.tr("Forget secret")
            icon.name: "edit-clear"
            visible: card.provider.has_secret
            onTriggered: App.act("ForgetSecret", [card.provider.id], "Providers1")
        },
        Kirigami.Action {
            text: App.tr("Change option…")
            icon.name: "configure"
            onTriggered: dialogs.option.open()
        },
        Kirigami.Action {
            text: App.tr("Create playlist")
            icon.name: "view-media-playlist"
            enabled: !!card.provider.source
            onTriggered: App.act("Create", [card.provider.id, "source:" + card.provider.source], "Playlists1", () => App.navigate(1, ""))
        },
        Kirigami.Action {
            text: App.tr("Show items")
            icon.name: "folder-pictures"
            enabled: !!card.provider.source
            onTriggered: App.navigate(0, "source:" + card.provider.source)
        },
        Kirigami.Action {
            text: App.tr("Remove…")
            icon.name: "edit-delete"
            onTriggered: dialogs.remover.open()
        }
    ]

    contentItem: Kirigami.FormLayout {
        QQC2.Label {
            Kirigami.FormData.label: App.tr("State:")
            text: Labels.value(card.provider.state)
            color: ["error", "needs_secret", "locked"].includes(card.provider.state) ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Error:")
            visible: !!card.provider.last_error
            text: card.provider.last_error ?? ""
            wrapMode: Text.Wrap
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Contacts:")
            text: card.provider.hosts.join(", ")
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Items:")
            text: App.tr("%1 (cache: %2)").arg(card.provider.items).arg(App.bytes(card.provider.cache_bytes))
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Last sync:")
            text: App.time(card.provider.last_sync) || App.tr("never")
        }

        QQC2.Label {
            Kirigami.FormData.label: App.tr("Next sync:")
            visible: !!card.provider.next_sync
            text: App.time(card.provider.next_sync)
        }
    }

    ProviderDialogs {
        id: dialogs

        provider: card.provider
        kind: card.kind
    }
}
