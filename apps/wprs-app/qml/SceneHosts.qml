import QtQuick

Item {
    id: hosts

    required property var input
    readonly property Component pairs: pairsEditor
    readonly property Component condition: conditionEditor
    readonly property Component texts: textsEditor
    readonly property Component meta: metaEditor
    readonly property Component styles: stylesEditor
    readonly property Component emitters: emittersEditor
    readonly property Component rules: rulesEditor

    visible: false

    Component {
        id: pairsEditor

        ScenePairs {
            host: hosts.input
        }
    }

    Component {
        id: conditionEditor

        SceneCondition {
            host: hosts.input
        }
    }

    Component {
        id: textsEditor

        SceneTexts {
            host: hosts.input
        }
    }

    Component {
        id: metaEditor

        SceneMeta {
            host: hosts.input
        }
    }

    Component {
        id: stylesEditor

        SceneStyles {
            host: hosts.input
        }
    }

    Component {
        id: rulesEditor

        SceneWakeRules {
            host: hosts.input
        }
    }

    Component {
        id: emittersEditor

        SceneEmitterList {
            host: hosts.input
        }
    }
}
