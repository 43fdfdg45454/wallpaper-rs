import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: bounds

    required property var owner
    required property var body
    readonly property var kinds: [["rotation", 180, App.tr("Bound its turn, in degrees")], ["translation", 100, App.tr("Bound how far its head slides, in pixels")]]

    function set(kind, value, gesture) {
        const args = {};
        args[kind] = value;
        owner.run(body.bone, args, gesture);
    }

    Repeater {
        model: bounds.kinds

        delegate: ColumnLayout {
            id: bound

            required property var modelData
            readonly property string kind: modelData[0]
            readonly property var held: bounds.body[kind] ?? null

            Layout.fillWidth: true

            QQC2.CheckBox {
                text: bound.modelData[2]
                checked: bound.held !== null
                onToggled: {
                    if (checked)
                        bounds.set(bound.kind, [-30, 30], false);
                    else
                        bounds.owner.run(bounds.body.bone, {
                            "free": [bound.kind]
                        });
                }
            }

            QQC2.RangeSlider {
                id: range

                Layout.fillWidth: true
                visible: bound.held !== null
                from: -bound.modelData[1]
                to: bound.modelData[1]
                stepSize: 1
                first.value: bound.held ? bound.held[0] : -30
                second.value: bound.held ? bound.held[1] : 30
                first.onMoved: bounds.set(bound.kind, [range.first.value, range.second.value], true)
                second.onMoved: bounds.set(bound.kind, [range.first.value, range.second.value], true)
            }
        }
    }
}
