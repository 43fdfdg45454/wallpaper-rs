import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: pick

    required property var input
    readonly property var page: input.page
    readonly property var assets: (page.view.assets ?? []).filter(a => a.kind === "image")
    readonly property var pictures: assets.filter(a => !a.media && !a.path.toLowerCase().endsWith(".gif"))
    readonly property var chosen: assets.find(a => a.id === input.value) ?? null

    function cover(kind) {
        const id = kind === "current" ? "cover" : "previous_cover";
        const known = assets.find(a => a.id === id);
        const placeholder = chosen && !chosen.media ? chosen.id : (pictures[0]?.id ?? "");
        if (!known || known.media !== kind)
            page.run("asset", known ? {
                "asset": id,
                "cover": kind
            } : {
                "asset": id,
                "cover": kind,
                "placeholder": placeholder
            });
        input.commit(id);
    }

    RowLayout {
        QQC2.Button {
            icon.name: "media-playback-start"
            text: App.tr("Cover of what plays")
            enabled: pick.pictures.length > 0 || pick.assets.some(a => a.id === "cover")
            onClicked: pick.cover("current")
        }

        QQC2.Button {
            text: App.tr("Previous cover")
            enabled: pick.pictures.length > 0 || pick.assets.some(a => a.id === "previous_cover")
            onClicked: pick.cover("previous")
        }
    }

    RowLayout {
        visible: pick.chosen !== null && !!pick.chosen.media

        QQC2.Label {
            text: App.tr("Shown while nothing plays")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: pick.pictures.map(p => p.id)
            currentIndex: pick.pictures.findIndex(p => p.path === pick.chosen?.path)
            onActivated: index => pick.page.run("asset", {
                "asset": pick.chosen.id,
                "placeholder": pick.pictures[index].id
            })
        }
    }
}
