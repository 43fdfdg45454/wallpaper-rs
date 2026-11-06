pragma Singleton
import QtQuick

QtObject {
    function at(list, option) {
        const key = JSON.stringify(option);
        return list.findIndex(item => JSON.stringify(item) === key);
    }

    function name(field, option) {
        if (option !== null && typeof option === "object" && option.layer !== undefined)
            return App.tr("Layer %1", option.layer);
        if (field.widget === "ref:language")
            return ManifestWords.language(option);
        if (["variant", "ref:effect"].includes(field.widget) || (field.widget === "enum" && !(field.names ?? []).length))
            return SceneWords.value(option);
        return (field.names ?? [])[at(field.options, option)] || (field.widget === "ref:trigger" ? SceneMediaWords.trigger(option) : String(option));
    }
}
