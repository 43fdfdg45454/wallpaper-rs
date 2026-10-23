import QtQuick
import QtTest
import "ui"
import "app" as App

Item {
    width: 400
    height: 320

    ScreenRow {
        id: row

        property var decided: []
        property var asked: []
        property var acts: []

        function i18n(text) {
            return text;
        }

        width: 400
        index: 0
        name: "X-1"
        info: ({
                "item": "A"
            })
        daemon: QtObject {
            signal updated(string topic, var ids)

            function query(member, args, iface, onReply) {
                row.asked.push(member);
                if (member === "Thumbnail")
                    return onReply([Qt.resolvedUrl("green.png").toString()]);
                onReply({
                    "item": {
                        "title": "Storm",
                        "package": {
                            "content": "mature",
                            "photosensitive": true,
                            "confirmed": row.decided
                        }
                    }
                });
            }

            function act(member, args, iface) {
                row.acts.push([iface, member].concat(args));
            }

            function url(path) {
                return path;
            }
        }
    }

    App.Thumbnail {
        id: shown

        property var acts: []

        y: 220
        width: 120
        height: 80
        item: "B"
        source: Qt.resolvedUrl("green.png")
        mature: acts.length === 0
        scene: false
        daemon: QtObject {
            function tr(text) {
                return text;
            }

            function act(member, args, iface) {
                shown.acts = shown.acts.concat([[iface, member].concat(args)]);
            }
        }
    }

    TestCase {
        function find(item, test) {
            return test(item) ? item : Array.from(item.data ?? []).map(child => find(child, test)).find(found => found) ?? null;
        }

        function decide(what) {
            row.decided = what;
            row.daemon.updated("library", ["A"]);
        }

        function test_mature_and_flashing() {
            tryVerify(() => row.thumb !== "");
            const image = find(row, item => item instanceof Image && String(item.source).endsWith("green.png"));
            const warning = find(row, item => String(item.text ?? "").startsWith("Flashes"));
            const reveal = find(row, item => String(item.text ?? "").startsWith("Mature content"));
            tryCompare(image, "status", Image.Ready);
            compare(row.asked, ["GetItem", "Thumbnail"]);
            compare([image.implicitHeight, warning.visible, reveal.visible], [12, true, true]);
            compare(warning.text, "Flashes or shows fast patterns that can trigger seizures");
            compare([reveal.activeFocusOnTab, reveal.width, reveal.height], [true, image.parent.width, image.parent.height]);
            reveal.forceActiveFocus();
            keyClick(Qt.Key_Space);
            compare(row.acts, [["Library1", "Confirm", ["A"], "mature"]]);
            mouseClick(image);
            compare(row.acts.length, 2);
            decide(["mature"]);
            tryCompare(image, "implicitHeight", 32);
            compare(reveal.visible, false);
            mouseClick(image);
            compare(row.acts.length, 2);
            decide(["mature", "photosensitive"]);
            tryCompare(warning, "text", "Flashes or shows fast patterns; you allowed it");
        }

        function test_the_app_reveals_with_a_key() {
            const image = find(shown, item => item instanceof Image);
            const reveal = find(shown, item => String(item.text ?? "").startsWith("Mature content"));
            tryCompare(image, "status", Image.Ready);
            compare([image.implicitHeight, reveal.visible, reveal.activeFocusOnTab], [12, true, true]);
            reveal.forceActiveFocus();
            keyClick(Qt.Key_Space);
            compare(shown.acts, [["Library1", "Confirm", ["B"], "mature"]]);
            tryCompare(image, "implicitHeight", 32);
            mouseClick(image);
            compare([reveal.visible, shown.acts.length], [false, 1]);
        }

        when: windowShown
    }
}
