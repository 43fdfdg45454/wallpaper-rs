import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.FormLayout {
    required property var host

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Kind:")
        text: [Labels.value(host.item.package?.kind === "scene" ? "scene" : host.item.kind), host.item.width ? host.item.width + "×" + host.item.height : ""].filter(t => t).join(" · ")
        textFormat: Text.PlainText
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("File:")
        Layout.maximumWidth: Kirigami.Units.gridUnit * 14
        text: host.item.path ?? ""
        textFormat: Text.PlainText
        wrapMode: Text.WrapAnywhere
    }

    QQC2.Label {
        readonly property var made: host.details.about?.platforms ?? []
        readonly property bool elsewhere: made.length > 0 && !made.includes("desktop") && !!host.item.error
        readonly property var refused: host.item.error ? host.details.about?.refused ?? [] : []
        Kirigami.FormData.label: App.tr("Problem:")
        visible: text !== ""
        text: host.item.missing ? App.tr("The file is missing") : elsewhere ? App.tr("This package is made for %1 only", made.map(Labels.value).join(", ")) : refused.length > 0 ? App.tr("This package needs a newer version of wallpaper-rs: it %1", refused.map(Labels.feature).join(", ")) : host.item.error ?? ""
        textFormat: Text.PlainText
        color: Kirigami.Theme.negativeTextColor
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Package:")
        visible: !!host.item.package
        text: host.item.package ? [host.item.package.id, host.item.package.version, host.item.package.license].join(" · ") : ""
        textFormat: Text.PlainText
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Installed from:")
        visible: !!host.details.installed
        Layout.maximumWidth: Kirigami.Units.gridUnit * 14
        text: host.details.installed ? [host.repo || host.details.installed.repo_id.slice(0, 8), host.details.installed.signer ? App.tr("signed by %1", host.details.installed.signer) : ""].filter(t => t).join("\n") : ""
        textFormat: Text.PlainText
        wrapMode: Text.WrapAnywhere
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Revoked:")
        visible: !!host.details.revoked
        text: Labels.value(host.details.revoked)
        textFormat: Text.PlainText
        color: Kirigami.Theme.negativeTextColor
    }

    QQC2.Label {
        Kirigami.FormData.label: App.tr("Credit:")
        visible: !!host.credit
        text: host.credit ? [host.credit.title, host.credit.attribution, host.credit.license, host.credit.link].filter(t => t).join("\n") : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
    }
}
