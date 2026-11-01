import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Page {
    id: page

    required property string folder
    property string starter
    property string heading
    property string source
    property alias editor: editor
    property alias tabs: panel.tabs
    property alias builder: builder
    property alias settings: panel.settings
    property string spot
    property var picked: []
    property bool moving
    property var brush: null
    property string rigging
    property var sculpt: null
    property string notice
    property real clock: editor.time
    readonly property var view: JSON.parse(editor.view || "{}")
    onViewChanged: SceneWords.own = view.effect_names ?? ({})
    readonly property var fields: JSON.parse(editor.fields || "[]")

    function run(action, args, gesture) {
        editor.run(action, JSON.stringify(args ?? {}), gesture === true);
    }

    function change(pointer, value, gesture) {
        const keyed = (view.tracks ?? []).some(track => track.pointer === pointer);
        const args = keyed ? {
            "time": editor.time
        } : {};
        run(keyed ? "keyframe" : "set", Object.assign(args, {
            "pointer": pointer,
            "value": value
        }), gesture);
    }

    function jump(pointer) {
        const own = (view.layers ?? []).concat(view.properties ?? [], view.presets ?? []).some(item => item.pointer === pointer);
        const parent = pointer.split("/").slice(0, -1).join("/").replace(/\/(params|values)$/, "");
        editor.select(own || parent.split("/").length < 3 ? pointer : parent);
        spot = pointer;
    }

    title: App.tr("Scene: %1", view.title ?? "")
    padding: 0
    Component.onCompleted: {
        if (starter)
            editor.create(folder, starter, heading);
        else
            editor.open(folder, source);
        if (App.checking)
            Qt.createComponent("SceneCheck.qml").createObject(page, {
                "page": page
            }).exercise();
    }
    actions: [
        Kirigami.Action {
            text: App.tr("Back")
            icon.name: "go-previous"
            onTriggered: {
                editor.autosave();
                App.navigate(7, "");
            }
        },
        Kirigami.Action {
            text: App.tr("Undo")
            icon.name: "edit-undo"
            shortcut: StandardKey.Undo
            tooltip: page.view.history?.undo ? App.tr("Undo: %1", page.view.history.undo) : ""
            enabled: (page.view.history?.done ?? []).length > 0
            onTriggered: page.run("undo")
        },
        Kirigami.Action {
            text: App.tr("Redo")
            icon.name: "edit-redo"
            shortcut: StandardKey.Redo
            tooltip: page.view.history?.redo ? App.tr("Redo: %1", page.view.history.redo) : ""
            enabled: (page.view.history?.undone ?? []).length > 0
            onTriggered: page.run("redo")
        },
        Kirigami.Action {
            text: App.tr("Save")
            icon.name: "document-save"
            shortcut: StandardKey.Save
            enabled: editor.dirty && !editor.busy
            onTriggered: editor.save()
        },
        Kirigami.Action {
            text: App.tr("Build and apply…")
            icon.name: "run-build"
            enabled: editor.ready && !editor.busy
            onTriggered: builder.open()
        },
        Kirigami.Action {
            text: App.tr("Show on the desktop")
            icon.name: "view-visible"
            checkable: true
            checked: editor.live
            enabled: editor.ready
            onTriggered: editor.preview(checked)
        }
    ]

    SceneEditor {
        id: editor

        onFinished: message => page.notice = message
    }

    Timer {
        interval: 30000
        running: editor.ready
        repeat: true
        onTriggered: editor.autosave()
    }

    Connections {
        target: page.Window.window

        function onVisibilityChanged(visibility) {
            editor.cover(visibility === Window.Minimized || visibility === Window.Hidden);
        }
    }

    header: SceneNotes {
        page: page
    }

    RowLayout {
        anchors.fill: parent
        enabled: editor.ready
        spacing: 0

        SceneSide {
            id: panel

            Layout.preferredWidth: Kirigami.Units.gridUnit * 14
            Layout.fillWidth: false
            Layout.fillHeight: true
            page: page
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredWidth: Kirigami.Units.gridUnit
            spacing: 0

            SceneCanvas {
                Layout.fillWidth: true
                Layout.fillHeight: true
                page: page
            }

            SceneTimeline {
                Layout.fillWidth: true
                Layout.fillHeight: false
                Layout.preferredHeight: Kirigami.Units.gridUnit * 9
                page: page
            }
        }

        ColumnLayout {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            Layout.fillWidth: false
            Layout.fillHeight: true

            SceneInspector {
                Layout.fillWidth: true
                Layout.fillHeight: true
                page: page
            }

            SceneProblems {
                Layout.fillWidth: true
                Layout.fillHeight: false
                Layout.preferredHeight: Kirigami.Units.gridUnit * ((page.view.problems ?? []).length ? 7 : 2)
                page: page
            }
        }
    }

    SceneBuild {
        id: builder

        page: page
    }
}
