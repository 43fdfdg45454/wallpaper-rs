import QtQuick
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

QQC2.ToolButton {
    id: bind

    required property var page
    required property var field
    readonly property var keys: field.source === "keys" ? field.value.keys ?? [] : []
    readonly property bool here: keys.some(key => Math.abs(key[0] - page.editor.time) < 0.001)
    readonly property bool listed: field.widget === "emitters"

    function bound(source) {
        page.run("set", {
            "pointer": field.pointer,
            "value": source
        });
    }

    function follow(prop) {
        if (!listed)
            return bound({
                "prop": prop.id
            });
        const now = JSON.parse(page.editor.value(prop.pointer + "/default") || "null");
        const map = {};
        map[now === null ? "null" : String(now)] = field.shown ?? field.default;
        bound({
            "prop": prop.id,
            "map": map
        });
    }

    function key(remove) {
        page.run("keyframe", {
            "pointer": field.pointer,
            "time": page.editor.time,
            "value": remove ? undefined : (field.shown ?? field.default),
            "remove": remove
        });
    }

    text: field.source === "keys" ? (here ? "◆" : "◈") : (field.source ? "⛓" : "◇")
    QQC2.ToolTip.text: field.source ? App.tr("Follows %1", SceneWords.binding(field.value)) : App.tr("Fixed value")
    QQC2.ToolTip.visible: hovered
    onClicked: menu.popup()

    QQC2.Menu {
        id: menu

        QQC2.MenuItem {
            text: App.tr("Keyframe here")
            enabled: !bind.field.source || bind.field.source === "keys"
            onTriggered: bind.key(false)
        }

        QQC2.MenuItem {
            text: App.tr("Remove the keyframe here")
            enabled: bind.here
            onTriggered: bind.key(true)
        }

        QQC2.MenuItem {
            text: App.tr("Edit the source…")
            enabled: bind.field.source !== ""
            onTriggered: bind.page.editor.select(bind.field.pointer)
        }

        QQC2.MenuItem {
            text: App.tr("Fixed value")
            enabled: bind.field.source !== ""
            onTriggered: bind.bound(bind.field.source === "keys" ? bind.field.shown : bind.field.default)
        }

        QQC2.Menu {
            title: App.tr("Follow a user property")

            Repeater {
                model: (bind.page.view.properties ?? []).filter(p => !["button", "group", "label", "divider"].includes(p.type))

                delegate: QQC2.MenuItem {
                    required property var modelData

                    text: modelData.label
                    onTriggered: bind.follow(modelData)
                }
            }
        }

        QQC2.Menu {
            title: App.tr("Follow the sound")
            enabled: !bind.listed

            Repeater {
                model: [["bass", App.tr("Bass")], ["mid", App.tr("Mids")], ["treble", App.tr("Treble")], ["level", App.tr("Level")], ["beat", App.tr("Beat")]]

                delegate: QQC2.MenuItem {
                    required property var modelData

                    text: modelData[1]
                    onTriggered: bind.bound({
                        "audio": modelData[0]
                    })
                }
            }
        }

        QQC2.Menu {
            title: App.tr("Follow the media player")
            enabled: media.count > 0 && !bind.listed

            Repeater {
                id: media

                model: SceneMediaWords.sources(bind.field.widget)

                delegate: QQC2.MenuItem {
                    required property var modelData

                    text: modelData[2] !== "" ? App.tr("%1 (now %2)", modelData[1], modelData[2]) : modelData[1]
                    onTriggered: bind.bound({
                        "media": modelData[0]
                    })
                }
            }
        }

        QQC2.Menu {
            title: App.tr("Follow the pointer")
            enabled: !bind.listed

            Repeater {
                model: [["x", App.tr("Horizontal position")], ["y", App.tr("Vertical position")], ["inside", App.tr("Over the screen")], ["speed", App.tr("Speed")]]

                delegate: QQC2.MenuItem {
                    required property var modelData

                    text: modelData[1]
                    onTriggered: bind.bound({
                        "cursor": modelData[0]
                    })
                }
            }
        }
    }
}
