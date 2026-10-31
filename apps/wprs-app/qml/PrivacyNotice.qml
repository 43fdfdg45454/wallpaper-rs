import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.InlineMessage {
    id: notice

    property var exposes: []
    property bool reduced
    readonly property var words: ({
            "zone": App.tr("your time zone"),
            "language": App.tr("your language"),
            "gpu": App.tr("what your graphics card can do"),
            "storage": App.tr("data it keeps on this computer"),
            "network": App.tr("the network, to reach the sites it names")
        })
    readonly property string used: App.tr("This web wallpaper uses %1. You can restrict each of them in its settings.", exposes.map(name => words[name] ?? name).join(", "))
    readonly property string weaker: App.tr("Its web engine runs without part of its sandbox on this computer.")

    visible: exposes.length > 0 || reduced
    type: reduced ? Kirigami.MessageType.Warning : Kirigami.MessageType.Information
    text: [exposes.length > 0 ? used : "", reduced ? weaker : ""].filter(t => t).join(" ")
}
