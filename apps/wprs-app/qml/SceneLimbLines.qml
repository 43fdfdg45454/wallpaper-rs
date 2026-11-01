import QtQuick
import org.kde.kirigami as Kirigami

Canvas {
    id: lines

    required property var owner
    readonly property real reach: Kirigami.Units.gridUnit * 2.5

    function line(ctx, a, b) {
        ctx.beginPath();
        ctx.moveTo(a[0] * width, a[1] * height);
        ctx.lineTo(b[0] * width, b[1] * height);
        ctx.stroke();
    }

    onPaint: {
        const ctx = getContext("2d"), unit = Kirigami.Units.gridUnit, rad = d => d * Math.PI / 180;
        ctx.reset();
        ctx.lineWidth = 2;
        ctx.setLineDash([4, 4]);
        ctx.strokeStyle = Kirigami.Theme.neutralTextColor;
        for (const c of owner.shown.ik ?? [])
            if (c.end && c.target)
                line(ctx, c.end, c.target);
        ctx.strokeStyle = Kirigami.Theme.highlightColor;
        for (const p of owner.pins)
            line(ctx, p.from, p.to);
        ctx.setLineDash([]);
        ctx.strokeStyle = Kirigami.Theme.positiveTextColor;
        for (const c of owner.shown.ik ?? [])
            for (const l of c.limits) {
                ctx.beginPath();
                ctx.arc(l.at[0] * width, l.at[1] * height, reach, rad(l.rest + l.min), rad(l.rest + l.max));
                ctx.stroke();
            }
        ctx.strokeStyle = Kirigami.Theme.negativeTextColor;
        ctx.strokeStyle = Kirigami.Theme.neutralTextColor;
        for (const b of owner.shown.bodies ?? [])
            if (b.rotation) {
                ctx.beginPath();
                ctx.arc(b.at[0] * width, b.at[1] * height, reach, rad(b.rest + b.rotation[0]), rad(b.rest + b.rotation[1]));
                ctx.stroke();
            }
        ctx.strokeStyle = Kirigami.Theme.negativeTextColor;
        for (const b of owner.shown.bodies ?? []) {
            const x = b.at[0] * width, y = b.at[1] * height, g = b.gravity * unit * 0.6, s = Math.sign(g) * 6;
            ctx.beginPath();
            ctx.arc(x, y, unit * 0.7, 0, 2 * Math.PI);
            ctx.moveTo(x, y);
            ctx.lineTo(x, y + g);
            ctx.lineTo(x - 4, y + g - s);
            ctx.moveTo(x, y + g);
            ctx.lineTo(x + 4, y + g - s);
            ctx.stroke();
        }
        owner.rims.forEach((rim, i) => {
            ctx.strokeStyle = i === 0 ? Kirigami.Theme.highlightColor : Kirigami.Theme.negativeTextColor;
            ctx.setLineDash(i === 0 ? [] : [3, 3]);
            rim[1].forEach(e => line(ctx, e[0], e[1]));
        });
    }
}
