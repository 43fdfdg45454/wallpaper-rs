import QtQuick
import QtQuick.Shapes
import org.kde.kirigami as Kirigami

Shape {
    id: outline

    required property string kind
    required property var spec
    required property var box
    required property var stretch
    property real grow: 0
    property bool dashed: false
    readonly property real w: Math.max(0, (box[0] + 2 * grow) * stretch[0])
    readonly property real h: Math.max(0, (box[1] + 2 * grow) * stretch[1])
    readonly property color ink: Kirigami.Theme.highlightColor
    readonly property color tint: dashed ? "transparent" : Qt.rgba(ink.r, ink.g, ink.b, 0.2)

    function corners(count, inner) {
        const out = [];
        const steps = inner > 0 ? count * 2 : count;
        for (let i = 0; i <= steps; i++) {
            const angle = -Math.PI / 2 + i * 2 * Math.PI / steps;
            const r = inner > 0 && i % 2 === 1 ? inner : 1;
            out.push(Qt.point(w / 2 + Math.cos(angle) * r * w / 2, h / 2 + Math.sin(angle) * r * h / 2));
        }
        return out;
    }

    function points() {
        if (kind === "polygon")
            return corners(Math.max(3, spec.sides ?? 3), 0);
        if (kind === "star")
            return corners(Math.max(2, spec.points ?? 5), spec.inner ?? 0.5);
        if (kind === "line") {
            const to = spec.to ?? [0, 0];
            return [Qt.point(w / 2 - to[0] / 2 * stretch[0], h / 2 - to[1] / 2 * stretch[1]), Qt.point(w / 2 + to[0] / 2 * stretch[0], h / 2 + to[1] / 2 * stretch[1])];
        }
        const r = kind === "rect" ? Math.min((spec.radius ?? 0) * Math.min(stretch[0], stretch[1]), w / 2, h / 2) : 0;
        return [Qt.point(r, 0), Qt.point(w - r, 0), Qt.point(w, r), Qt.point(w, h - r), Qt.point(w - r, h), Qt.point(r, h), Qt.point(0, h - r), Qt.point(0, r), Qt.point(r, 0)];
    }

    x: (parent.width - w) / 2
    y: (parent.height - h) / 2
    width: w
    height: h

    ShapePath {
        readonly property bool round: ["ellipse", "ring"].includes(outline.kind)

        strokeColor: round ? outline.ink : "transparent"
        strokeWidth: 2
        strokeStyle: outline.dashed ? ShapePath.DashLine : ShapePath.SolidLine
        fillColor: round ? outline.tint : "transparent"

        PathAngleArc {
            centerX: outline.w / 2
            centerY: outline.h / 2
            radiusX: outline.w / 2
            radiusY: outline.h / 2
            startAngle: 0
            sweepAngle: 360
        }
    }

    ShapePath {
        readonly property bool round: ["ellipse", "ring"].includes(outline.kind)

        strokeColor: round ? "transparent" : outline.ink
        strokeWidth: outline.kind === "line" ? Math.max(2, (outline.spec.width ?? 1) * outline.stretch[1]) : 2
        strokeStyle: outline.dashed ? ShapePath.DashLine : ShapePath.SolidLine
        fillColor: round || outline.kind === "line" ? "transparent" : outline.tint

        PathPolyline {
            path: outline.points()
        }
    }
}
