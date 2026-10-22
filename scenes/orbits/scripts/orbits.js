import { props, scene } from "wprs";

const planets = [
    ["inner", 210, 1.6, 0.4],
    ["middle", 330, 1.0, 2.6],
    ["outer", 470, 0.6, 4.4],
];
const tilt = 0.42;

function place(id, center, radius, angle) {
    const at = [center[0] + Math.cos(angle) * radius, center[1] + Math.sin(angle) * radius * tilt];
    scene.layer(id).position = at;
    const size = 0.85 + Math.sin(angle) * 0.15;
    scene.layer(id).scale = [size, size];
    return at;
}

export function update({ time }) {
    const turn = time * props.get("speed") * 0.25;
    let outer = [960, 540];
    for (const [id, radius, rate, phase] of planets) {
        outer = place(id, [960, 540], radius, turn * rate + phase);
    }
    place("moon", outer, 70, turn * 4);
}
