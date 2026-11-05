import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: field

    readonly property string key: "surfaces.layer_namespace"
    readonly property string chosen: String((App.state.options ?? {})[key] ?? "auto")
    readonly property string own: chosen === "auto" ? "" : chosen
    readonly property var shell: App.state.layer_shell ?? {}
    readonly property var sources: ({
            "option": App.tr("the option the daemon was started with"),
            "environment": App.tr("the WPRS_LAYER_NAMESPACE variable"),
            "config": App.tr("this setting"),
            "automatic": App.tr("the automatic choice for this desktop"),
            "default": App.tr("the usual name")
        })

    function apply(text) {
        App.act("SetOption", [key, text || "auto"]);
    }

    RowLayout {
        QQC2.TextField {
            id: name

            Layout.preferredWidth: Kirigami.Units.gridUnit * 16
            Accessible.name: App.tr("Surface name")
            placeholderText: App.tr("Automatic")
            text: field.own
            maximumLength: 64
            validator: RegularExpressionValidator {
                regularExpression: /[A-Za-z0-9._-]*/
            }
            onAccepted: field.apply(text)
        }

        QQC2.Button {
            text: App.tr("Apply")
            enabled: name.text !== field.own
            onClicked: field.apply(name.text)
        }

        QQC2.Button {
            text: App.tr("Automatic")
            visible: field.own !== ""
            onClicked: {
                name.text = "";
                field.apply("");
            }
        }
    }

    QQC2.Label {
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: App.tr("The name the wallpaper surfaces give the compositor. Change it if the overview of your compositor shows the wallpaper black: some only show it there under a name they know. Empty chooses automatically.")
    }

    QQC2.Label {
        visible: !!field.shell.namespace
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: App.tr("In use: %1, from %2.", field.shell.namespace ?? "", field.sources[field.shell.from] ?? field.shell.from ?? "")
    }

    QQC2.Label {
        visible: field.shell.restart === true
        Layout.maximumWidth: Kirigami.Units.gridUnit * 30
        wrapMode: Text.Wrap
        text: App.tr("The new name takes effect the next time the daemon starts.")
        color: Kirigami.Theme.neutralTextColor
    }
}
