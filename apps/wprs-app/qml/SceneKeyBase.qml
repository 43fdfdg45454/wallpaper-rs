import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: base

    required property var curve
    readonly property var track: curve.track
    readonly property bool relative: track !== null && track.base !== undefined && track.base !== null

    function keyed(args) {
        args.pointer = track.pointer;
        args.time = curve.from[0];
        curve.timeline.page.run("keyframe", args);
    }

    QQC2.Button {
        visible: curve.shaped
        icon.name: "draw-line"
        text: App.tr("Straighten the curve")
        onClicked: {
            base.keyed({
                "ease": "linear"
            });
            curve.timeline.page.run("keyframe", {
                "pointer": base.track.pointer,
                "time": curve.to[0],
                "ease": curve.to[2] && typeof curve.to[2] === "string" ? curve.to[2] : "linear"
            });
        }
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: base.relative ? App.tr("The keyframes are added to this base value:") : App.tr("Make the keyframes relative by adding them to a base value:")
    }

    RowLayout {
        enabled: base.track !== null && curve.from !== null

        QQC2.TextField {
            id: value

            Layout.fillWidth: true
            placeholderText: App.tr("A number, [x, y] or a color")
            text: base.relative ? JSON.stringify(base.track.base) : ""
        }

        QQC2.Button {
            text: App.tr("Set the base")
            enabled: value.text.trim() !== ""
            onClicked: {
                let parsed = value.text.trim();
                try {
                    parsed = JSON.parse(parsed);
                } catch (e) {}
                base.keyed({
                    "base": parsed
                });
            }
        }

        QQC2.Button {
            text: App.tr("Absolute")
            enabled: base.relative
            onClicked: base.keyed({
                "absolute": true
            })
        }
    }
}
