import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import QtQuick.Dialogs
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    required property var page
    property var chosen: []
    property var thumbnails: ({})
    property var titles: ({})
    property string thumbnail: ""
    property string picking: ""
    property var outcome: null

    function offered() {
        const layers = (page.view.layers ?? []).map(l => ({
                    "key": "layer:" + l.id,
                    "label": l.label,
                    "kind": "layer"
                }));
        const systems = (page.view.systems ?? []).map(id => ({
                    "key": "particles:" + id,
                    "label": id,
                    "kind": "particles"
                }));
        const scripts = (JSON.parse(page.editor.value("/scene/scripts") || "null") ?? []).map(s => ({
                    "key": "script:" + s.path,
                    "label": s.path,
                    "kind": "script"
                }));
        const effects = Object.keys(JSON.parse(page.editor.value("/scene/effects") || "null") ?? {}).map(id => ({
                    "key": "effect:" + id,
                    "label": SceneWords.effect(id),
                    "kind": "effect"
                }));
        return layers.concat(systems, scripts, effects);
    }

    function toggle(key, on) {
        const rest = chosen.filter(k => k !== key);
        chosen = on ? rest.concat([key]) : rest;
    }

    function make() {
        if (page.editor.dirty)
            page.editor.save();
        outcome = JSON.parse(shelf.offer(page.editor.folder, JSON.stringify({
            "out": out.text,
            "parts": chosen,
            "id": id.text,
            "title": name.text,
            "license": license.text,
            "attribution": attribution.text,
            "thumbnail": thumbnail,
            "thumbnails": thumbnails,
            "titles": titles
        })) || "null");
    }

    title: App.tr("Make an assets package")
    standardButtons: Kirigami.Dialog.Close
    onOpened: {
        const layer = (page.view.layers ?? []).find(l => l.pointer === page.editor.selected);
        chosen = layer ? ["layer:" + layer.id] : [];
        outcome = null;
    }

    Shelf {
        id: shelf
    }

    ColumnLayout {
        Layout.preferredWidth: Kirigami.Units.gridUnit * 28

        Kirigami.FormLayout {
            QQC2.TextField {
                id: name

                Kirigami.FormData.label: App.tr("Title:")
                text: (dialog.page.view.title ?? "") + " " + App.tr("parts")
            }

            QQC2.TextField {
                id: id

                Kirigami.FormData.label: App.tr("Package id:")
                text: (dialog.page.view.id ?? "org.example.scene") + ".parts"
            }

            QQC2.TextField {
                id: license

                Kirigami.FormData.label: App.tr("License:")
                text: dialog.page.view.license ?? "CC-BY-4.0"
            }

            QQC2.TextField {
                id: attribution

                Kirigami.FormData.label: App.tr("Credit as:")
                placeholderText: App.tr("Your name, as other authors will credit you")
            }

            QQC2.TextField {
                id: out

                Kirigami.FormData.label: App.tr("Folder:")
                text: dialog.page.editor.folderFor(name.text)
            }

            RowLayout {
                Kirigami.FormData.label: App.tr("Preview:")

                Image {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 6
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 4
                    fillMode: Image.PreserveAspectFit
                    source: dialog.thumbnail ? App.url(dialog.thumbnail) : ""
                }

                QQC2.Button {
                    icon.name: "insert-image"
                    text: App.tr("Pick a picture…")
                    onClicked: {
                        dialog.picking = "";
                        pictures.open();
                    }
                }
            }
        }

        SceneRepeater {
            id: listed

            rows: dialog.offered()

            delegate: SceneOfferPart {
                required property int index
                readonly property var modelData: listed.rows[index]

                Layout.fillWidth: true
                part: modelData
                chosen: dialog.chosen.includes(modelData.key)
                picture: dialog.thumbnails[modelData.key] ?? ""
                onToggled: on => dialog.toggle(modelData.key, on)
                onRenamed: text => dialog.titles = Object.assign({}, dialog.titles, Object.fromEntries([[modelData.key, text]]))
                onPick: {
                    dialog.picking = modelData.key;
                    pictures.open();
                }
            }
        }

        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            color: dialog.outcome?.error ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
            visible: dialog.outcome !== null
            text: dialog.outcome?.error ?? App.tr("The assets package is in %1; build it from its own editor.", dialog.outcome?.folder ?? "")
        }

        RowLayout {
            QQC2.Button {
                icon.name: "document-save"
                text: App.tr("Make the package")
                enabled: dialog.chosen.length > 0 && name.text && id.text && license.text && attribution.text
                onClicked: dialog.make()
            }

            QQC2.Button {
                icon.name: "document-edit"
                text: App.tr("Open it in the editor")
                visible: !!dialog.outcome?.folder
                onClicked: App.compose(dialog.outcome.folder, "", "")
            }
        }
    }

    FileDialog {
        id: pictures

        nameFilters: [App.tr("Pictures") + " (*.png *.jpg *.jpeg *.webp *.gif *.bmp)"]
        onAccepted: {
            const path = App.path(selectedFile);
            if (dialog.picking)
                dialog.thumbnails = Object.assign({}, dialog.thumbnails, Object.fromEntries([[dialog.picking, path]]));
            else
                dialog.thumbnail = path;
        }
    }
}
