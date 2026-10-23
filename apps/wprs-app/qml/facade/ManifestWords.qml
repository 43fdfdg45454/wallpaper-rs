pragma Singleton
import QtQuick

QtObject {
    readonly property var words: ({
            "animated": App.tr("Animated preview"),
            "attribution": App.tr("Attribution"),
            "author": App.tr("Author"),
            "authors": App.tr("Authors"),
            "changes": App.tr("Changes in this version"),
            "content": App.tr("Content"),
            "credits": App.tr("Credits"),
            "default_locale": App.tr("Default language"),
            "derived_from": App.tr("Remade from"),
            "description": App.tr("Description"),
            "descriptors": App.tr("Reasons"),
            "display": App.tr("Framing"),
            "donate": App.tr("Donate"),
            "engine": App.tr("Version of wallpaper-rs"),
            "entry": App.tr("Entry file"),
            "features": App.tr("Features"),
            "homepage": App.tr("Home page"),
            "inputs": App.tr("May ask for"),
            "kind": App.tr("Kind"),
            "license": App.tr("License"),
            "links": App.tr("Links"),
            "links.source": App.tr("Source"),
            "manifest": App.tr("Package"),
            "paths": App.tr("Files"),
            "photosensitive": App.tr("Flashes or fast patterns"),
            "playback": App.tr("Playback"),
            "presets": App.tr("Presets"),
            "preview": App.tr("Preview"),
            "previews": App.tr("Previews"),
            "properties": App.tr("Properties"),
            "rating": App.tr("Rating"),
            "requires": App.tr("Requirements"),
            "strings": App.tr("Translated texts"),
            "support": App.tr("Support"),
            "tags": App.tr("Tags"),
            "thumbnail": App.tr("Thumbnail"),
            "title": App.tr("Title"),
            "url": App.tr("Address"),
            "values": App.tr("Values"),
            "version": App.tr("Version")
        })
    readonly property var values: ({
            "derived": App.tr("Remade from another work"),
            "general": App.tr("For everyone"),
            "gore": App.tr("Gore"),
            "language": App.tr("Strong language"),
            "mature": App.tr("Mature"),
            "original": App.tr("Original work"),
            "sexual": App.tr("Sexual content"),
            "violence": App.tr("Violence")
        })
    readonly property var inputs: ({
            "audio": App.tr("The sound"),
            "clicks": App.tr("The clicks"),
            "media": App.tr("What the media player plays"),
            "network": App.tr("The network"),
            "pointer": App.tr("The pointer")
        })
}
