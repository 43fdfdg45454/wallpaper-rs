pragma Singleton
import QtQuick

QtObject {
    readonly property var fields: ({
            "order": App.tr("Direction"),
            "top_range": App.tr("Top list of"),
            "resolutions": App.tr("Exact resolutions"),
            "ratios": App.tr("Aspect ratios"),
            "colors": App.tr("Colors"),
            "similar": App.tr("Similar to wallpaper"),
            "tag": App.tr("Tag number"),
            "uploader": App.tr("Uploaded by"),
            "collection": App.tr("Collection"),
            "page": App.tr("First page"),
            "pages": App.tr("Pages per sync"),
            "max_items": App.tr("Most pictures"),
            "categories.general": App.tr("General"),
            "categories.anime": App.tr("Anime"),
            "categories.people": App.tr("People"),
            "purity.sfw": App.tr("Safe"),
            "purity.sketchy": App.tr("Sketchy"),
            "purity.nsfw": App.tr("Adult")
        })
    readonly property var values: ({
            "date_added": App.tr("Newest"),
            "relevance": App.tr("Relevance"),
            "random": App.tr("Random"),
            "views": App.tr("Most viewed"),
            "favorites": App.tr("Most favorited"),
            "toplist": App.tr("Top list"),
            "hot": App.tr("Hot"),
            "desc": App.tr("Descending"),
            "asc": App.tr("Ascending"),
            "1d": App.tr("Last day"),
            "3d": App.tr("Last 3 days"),
            "1w": App.tr("Last week"),
            "1M": App.tr("Last month"),
            "3M": App.tr("Last 3 months"),
            "6M": App.tr("Last 6 months"),
            "1y": App.tr("Last year"),
            "landscape": App.tr("Any landscape"),
            "portrait": App.tr("Any portrait")
        })
}
