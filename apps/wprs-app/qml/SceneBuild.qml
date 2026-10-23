import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Kirigami.Dialog {
    id: dialog

    required property var page
    readonly property bool ready: !page.editor.dirty && !page.editor.busy && !(page.view.problems ?? []).some(problem => !problem.warning)

    title: App.tr("Build and apply")
    padding: Kirigami.Units.largeSpacing
    preferredWidth: Kirigami.Units.gridUnit * 28
    standardButtons: Kirigami.Dialog.Close

    ColumnLayout {
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            visible: !dialog.ready
            text: App.tr("Save the scene and solve its problems before building it.")
        }

        Kirigami.FormLayout {
            Layout.fillWidth: true

            QQC2.TextField {
                id: out

                Kirigami.FormData.label: App.tr("Package file:")
                placeholderText: App.tr("Next to the folder, named after the id and version")
            }

            RowLayout {
                Kirigami.FormData.label: App.tr("Author key:")

                QQC2.TextField {
                    id: key

                    Layout.fillWidth: true
                    placeholderText: App.tr("None: the package is not signed")
                }

                QQC2.Button {
                    icon.name: "document-open"
                    text: App.tr("Choose…")
                    onClicked: keys.open()
                }
            }

            Kirigami.PasswordField {
                id: password

                Kirigami.FormData.label: App.tr("Key password:")
                enabled: key.text !== ""
            }

            QQC2.Button {
                text: App.tr("Build the package")
                icon.name: "run-build"
                enabled: dialog.ready
                onClicked: {
                    dialog.page.editor.build(out.text, key.text, password.text);
                    password.text = "";
                }
            }

            QQC2.ComboBox {
                id: target

                Kirigami.FormData.label: App.tr("Screen:")
                model: [App.tr("Every screen")].concat(App.outputs)
            }

            QQC2.Button {
                text: App.tr("Apply to the screen")
                icon.name: "preferences-desktop-wallpaper"
                enabled: dialog.ready
                onClicked: dialog.page.editor.apply(target.currentIndex > 0 ? App.outputs[target.currentIndex - 1] : "")
            }
        }
    }

    FileDialog {
        id: keys

        nameFilters: [App.tr("Secret keys") + " (*.key)"]
        onAccepted: key.text = App.path(selectedFile)
    }
}
