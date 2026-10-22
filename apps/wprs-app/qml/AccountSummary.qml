import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.FormLayout {
    id: summary

    required property var account

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Server:")
        text: [summary.account.name, summary.account.url].filter(t => t).join(" · ")
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Account:")
        text: summary.account.handle ?? "–"
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("This computer:")
        text: summary.account.device ? App.tr("%1, named %2 for your devices").arg(summary.account.device).arg(summary.account.label) : "–"
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("State:")
        text: App.value(summary.account.state)
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Last sync:")
        text: App.time(summary.account.last_sync) || "–"
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Storage:")
        text: App.tr("%1 of %2").arg(App.bytes(summary.account.used_bytes ?? 0)).arg(App.bytes(summary.account.quota_bytes ?? 0))
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Error:")
        visible: !!summary.account.last_error
        text: summary.account.last_error ?? ""
        color: Kirigami.Theme.negativeTextColor
        wrapMode: Text.Wrap
    }
}
