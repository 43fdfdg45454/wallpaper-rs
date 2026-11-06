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
    property alias builder: builder
    property alias tabs: side
    property string spot
    property string notice
    property real clock: 0
    readonly property var view: JSON.parse(editor.view || "{}")
    readonly property var fields: JSON.parse(editor.fields || "[]")
    readonly property var manifest: page.view.id === undefined ? ({}) : JSON.parse(editor.value("/manifest") || "{}")
    readonly property string kind: view.kind ?? ""

    function run(action, args, gesture) {
        editor.run(action, JSON.stringify(args ?? {}), gesture === true);
    }

    function change(pointer, value, gesture) {
        run("set", {
            "pointer": pointer,
            "value": value
        }, gesture);
    }

    function manifestEdit(what, args) {
        const edit = {};
        edit[what] = args;
        run("manifest", edit);
    }

    function jump(pointer) {
        editor.select(pointer);
        spot = pointer;
    }

    title: App.tr("Package: %1", view.title ?? "")
    padding: 0
    Component.onCompleted: {
        if (starter)
            editor.create(folder, starter, heading);
        else
            editor.open(folder, source);
        editor.select("/manifest");
        if (App.checking)
            Qt.createComponent("PackageCheck.qml").createObject(page, {
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
            enabled: (page.view.history?.done ?? []).length > 0
            onTriggered: page.run("undo")
        },
        Kirigami.Action {
            text: App.tr("Redo")
            icon.name: "edit-redo"
            shortcut: StandardKey.Redo
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

    header: SceneNotes {
        page: page
    }

    RowLayout {
        anchors.fill: parent
        enabled: editor.ready
        spacing: 0

        ColumnLayout {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 14
            Layout.fillWidth: false
            Layout.fillHeight: true

            QQC2.TabBar {
                id: side

                Layout.fillWidth: true

                QQC2.TabButton {
                    text: App.tr("Files")
                }
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: side.currentIndex

                PackageFiles {
                    page: page
                }
            }
        }

        QQC2.ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.preferredWidth: Kirigami.Units.gridUnit
            contentWidth: availableWidth
            clip: true

            PackageSections {
                width: parent.width
                page: page
            }
        }

        ColumnLayout {
            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            Layout.fillWidth: false
            Layout.fillHeight: true

            PackageInspector {
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
