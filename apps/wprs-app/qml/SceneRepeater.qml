import QtQuick

Repeater {
    id: repeater

    property var rows: []
    readonly property string identity: (rows ?? []).length + "\u0001" + (rows ?? []).map(row => repeater.key(row)).join("\u0001")

    function key(row) {
        if (row === null || row === undefined)
            return "";
        if (Array.isArray(row))
            return key(row[0]);
        if (typeof row !== "object")
            return String(row);
        const named = row.pointer ?? row.id ?? row.key ?? row.name ?? row.ref ?? row.bone ?? row.kind ?? row.label ?? "";
        return typeof named === "object" ? JSON.stringify(named) : String(named);
    }

    function rebuild() {
        model = 0;
        model = (rows ?? []).length;
    }

    model: (rows ?? []).length
    onIdentityChanged: rebuild()
}
