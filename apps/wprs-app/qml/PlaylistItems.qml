import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: entries

    required property string uid
    required property bool manual
    required property var choices

    function edit(member, args) {
        App.act(member, [uid].concat(args), "Playlists1", null, error => {
            if (!notice)
                return;
            if (error.name === App.conflict) {
                notice.visible = true;
                entries.load();
            }
        });
    }

    function look() {
        const last = list.indexAt(0, list.contentY + list.height - 1);
        items.visible(Math.max(0, list.indexAt(0, list.contentY)), last < 0 ? list.count - 1 : last);
    }

    function load() {
        if (uid)
            items.playlist(uid);
    }

    onUidChanged: load()
    Component.onCompleted: load()

    QQC2.TabBar {
        id: tabs

        Layout.fillWidth: true

        QQC2.TabButton {
            text: App.tr("Items (%1)", items.total)
        }

        QQC2.TabButton {
            text: App.tr("History")
        }
    }

    Kirigami.InlineMessage {
        id: notice

        Layout.fillWidth: true
        type: Kirigami.MessageType.Warning
        visible: false
        showCloseButton: true
        text: App.tr("The playlist changed meanwhile; it was reloaded.")
    }

    StackLayout {
        Layout.fillWidth: true
        Layout.fillHeight: true
        currentIndex: tabs.currentIndex

        QQC2.ScrollView {
            ListView {
                id: list

                model: ItemModel {
                    id: items
                }
                onContentYChanged: entries.look()
                onCountChanged: entries.look()
                onHeightChanged: entries.look()

                delegate: QQC2.ItemDelegate {
                    id: row

                    required property int index
                    required property string uid
                    required property string title
                    required property string thumb
                    required property string color
                    required property string info
                    required property bool missing
                    required property bool mature
                    readonly property bool veiled: mature && !App.unveiled

                    width: list.width
                    contentItem: RowLayout {
                        Kirigami.ListItemDragHandle {
                            visible: entries.manual
                            listItem: row
                            listView: list
                            incrementalMoves: false
                            onMoveRequested: (from, to) => entries.edit("MoveItem", [from, to, items.uid(from)])
                        }

                        Rectangle {
                            Layout.preferredWidth: Kirigami.Units.gridUnit * 3
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 2
                            color: row.color || Kirigami.Theme.alternateBackgroundColor

                            Image {
                                anchors.fill: parent
                                source: row.thumb
                                asynchronous: true
                                fillMode: Image.PreserveAspectCrop
                                sourceSize.width: row.veiled ? 12 : width * 2
                                sourceSize.height: row.veiled ? 12 : height * 2
                            }
                        }

                        QQC2.Label {
                            Layout.fillWidth: true
                            text: row.title + (row.info ? " · " + row.info : "")
                            textFormat: Text.PlainText
                            color: row.missing ? Kirigami.Theme.disabledTextColor : Kirigami.Theme.textColor
                            elide: Text.ElideRight
                        }

                        QQC2.ToolButton {
                            visible: entries.manual
                            icon.name: "list-remove"
                            text: App.tr("Remove")
                            display: QQC2.AbstractButton.IconOnly
                            onClicked: entries.edit("RemoveEntries", [[row.index], [row.uid]])
                        }
                    }
                }
            }
        }

        History {
            choices: entries.choices
        }
    }
}
