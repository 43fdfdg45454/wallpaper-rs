import QtQuick
import QtQuick.Shapes
import org.kde.kirigami as Kirigami

Shape {
    id: lean

    required property var gizmo
    readonly property var corners: gizmo.modelData.corners ?? null

    parent: gizmo.parent
    anchors.fill: parent
    z: gizmo.z
    visible: corners !== null

    ShapePath {
        strokeColor: lean.gizmo.chosen ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
        strokeWidth: lean.gizmo.chosen ? 2 : 1
        strokeStyle: ShapePath.DashLine
        fillColor: "transparent"

        PathPolyline {
            path: (lean.corners ?? []).concat(lean.corners ? [lean.corners[0]] : []).map(c => Qt.point(c[0] * lean.gizmo.ratio, c[1] * lean.gizmo.ratio))
        }
    }
}
