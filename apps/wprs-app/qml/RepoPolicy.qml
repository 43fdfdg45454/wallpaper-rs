import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

GridLayout {
    id: policy

    property var current: null

    function reset() {
        const p = current?.policy ?? {};
        online.checked = p.online_authors ?? false;
        signed.checked = p.require_author_signature ?? false;
        days.value = p.root_days ?? 365;
        entries.value = p.entry_max_days ?? 14;
        size.value = Math.round((p.max_package_size ?? 1073741824) / 1048576);
        mirrors.text = (p.mirrors ?? []).join(", ");
        shards.value = p.shard_bits ?? 4;
    }

    function written() {
        const bytes = current?.policy?.max_package_size ?? 1073741824;
        const out = {
            "online_authors": online.checked,
            "require_author_signature": signed.checked,
            "root_days": days.value,
            "entry_max_days": entries.value,
            "mirrors": mirrors.text.split(",").map(m => m.trim()).filter(m => m !== ""),
            "shard_bits": shards.value
        };
        if (size.value !== Math.round(bytes / 1048576))
            out.max_package_size = size.value * 1048576;
        return out;
    }

    columns: 2

    QQC2.CheckBox {
        id: online

        Layout.columnSpan: 2
        text: App.tr("Let authors register online: clients trust per package the key that signed its first version")
    }

    QQC2.CheckBox {
        id: signed

        Layout.columnSpan: 2
        text: App.tr("Require author signatures")
    }

    QQC2.Label {
        text: App.tr("Days the root lasts")
    }

    QQC2.SpinBox {
        id: days

        from: 30
        to: 1096
        editable: true
    }

    QQC2.Label {
        text: App.tr("Longest life of a signed index, in days")
    }

    QQC2.SpinBox {
        id: entries

        from: 1
        to: 90
        editable: true
    }

    QQC2.Label {
        text: App.tr("Largest package, in MiB")
    }

    QQC2.SpinBox {
        id: size

        from: 1
        to: 65536
        editable: true
    }

    QQC2.Label {
        text: App.tr("HTTPS mirrors, separated by commas")
    }

    QQC2.TextField {
        id: mirrors

        Layout.fillWidth: true
    }

    QQC2.Label {
        text: App.tr("Bits of the index shards")
    }

    QQC2.SpinBox {
        id: shards

        from: 0
        to: 8
    }
}
