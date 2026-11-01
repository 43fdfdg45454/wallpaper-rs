import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function run(action, args) {
        page.run(action, Object.assign({
            "layer": "puppet"
        }, args));
        if (page.editor.error !== "")
            throw new Error(action + ": " + page.editor.error);
    }

    function exercise() {
        const word = v => [v & 127, v >> 8, 0, 0];
        const head = [66, 77].concat(word(102), word(0), word(54), word(40), word(4), word(4), [1, 0, 24, 0], word(0), word(48), Array(16).fill(0));
        const bytes = head.slice();
        for (let i = 0; i < 16; i++)
            bytes.push(32, 64, 96);
        if (!App.writeText(App.url(page.editor.folder + "/pic.bmp"), String.fromCharCode(...bytes)))
            throw new Error("the check could not write a picture");
        page.run("set", {
            "pointer": "/scene/assets/pic",
            "value": {
                "path": "pic.bmp"
            }
        });
        page.run("mesh_from_picture", {
            "image": "pic",
            "cells": 4,
            "id": "puppet"
        });
        run("bone", {
            "name": "root",
            "at": [0.2, 0.5]
        });
        run("bone", {
            "name": "arm",
            "parent": "root",
            "at": [0.7, 0.5]
        });
        page.editor.select(page.mesh.pointer("puppet"));
        page.mesh.layer = "puppet";
        page.mesh.bone = "arm";
        page.mesh.tool("puppet", "weights", {
            "mode": "add",
            "by": 0.5,
            "radius": 0.2,
            "falloff": 0.5,
            "lock": []
        });
        page.mesh.view = "owner";
        run("weights", {
            "bone": "arm",
            "points": [[0.5, 0.5]],
            "mode": "smooth",
            "by": 0.5
        });
        if (!(page.mesh.ask("puppet", {
                "weights": "arm"
            }).heat ?? "").startsWith("data:image/png;base64,"))
            throw new Error("the heat view has no picture");
        run("auto_weights", {});
        page.mesh.tool("puppet", "weights", null);
        run("clip", {
            "id": "wave",
            "add": true
        });
        page.mesh.clip = "wave";
        run("key", {
            "clip": "wave",
            "time": 1,
            "bone": "arm",
            "rotation": 30,
            "ease": "cubic"
        });
        page.mesh.pose = {
            "bone": "arm",
            "rotation": 45,
            "position": [0, 0],
            "scale": [1, 1],
            "changed": ["rotation"]
        };
        const keys = page.mesh.ask("puppet", {
            "keys": "wave"
        });
        if (!keys || !keys.rows.some(r => r.name === "arm") || page.mesh.ask("puppet", {
            "pose": {
                "clip": "wave",
                "time": 1
            }
        }).bones.length !== 2)
            throw new Error("the clip does not list its keys and pose: " + JSON.stringify(keys));
        page.mesh.clip = "";
        run("bone", {
            "name": "arm",
            "to": "limb"
        });
        page.run("undo");
        page.run("redo");
        if (page.mesh.ask("puppet").bones[1]?.name !== "limb")
            throw new Error("the bone was not renamed");
        Qt.createComponent("SceneLimbCheck.qml").createObject(page, {
            "page": page
        }).exercise();
        page.editor.select("/scene/layers/0");
    }
}
