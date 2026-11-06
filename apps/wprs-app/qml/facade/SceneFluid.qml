pragma Singleton
import QtQuick
import org.kde.kirigami as Kirigami

QtObject {
    function escaped(text) {
        return String(text).replace(/~/g, "~0").replace(/\//g, "~1");
    }

    function reach(strength) {
        return Kirigami.Units.gridUnit * (0.8 + Math.min(strength ?? 0, 64) / 16);
    }

    function aimed(from, x, y) {
        const dx = x - from[0], dy = y - from[1];
        const strength = (Math.hypot(dx, dy) / Kirigami.Units.gridUnit - 0.8) * 16;
        return {
            "angle": Math.round(Math.atan2(dy, dx) * 180 / Math.PI),
            "strength": Math.round(Math.max(0, Math.min(64, strength)) * 10) / 10
        };
    }
}
