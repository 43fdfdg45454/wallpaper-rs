use super::super::item::linear;
use bytemuck::{Pod, Zeroable};
use wprs_scene::{Beam, Color, Eye, Frame, Lamp, MAX_LIGHTS, Quality, Stage3d, unit3};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Light3 {
    pub(crate) at: [f32; 4],
    pub(crate) color: [f32; 4],
    pub(crate) dir: [f32; 4],
    pub(crate) more: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Scene3 {
    pub(crate) clip: [f32; 16],
    pub(crate) eye: [f32; 4],
    pub(crate) forward: [f32; 4],
    pub(crate) ambient: [f32; 4],
    pub(crate) sky: [f32; 4],
    pub(crate) up: [f32; 4],
    pub(crate) slice: [f32; 4],
    pub(crate) wind: [f32; 4],
    pub(crate) fade: [f32; 4],
    pub(crate) lights: [Light3; MAX_LIGHTS],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Shadows {
    pub(crate) mats: [[f32; 16]; 12],
    pub(crate) info: [[f32; 4]; 2],
    pub(crate) at: [[f32; 4]; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct DrawSlots {
    pub(crate) first: [f32; 4],
    pub(crate) slots: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Object {
    pub(crate) model: [f32; 16],
    pub(crate) normal: [f32; 16],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Finish3 {
    pub(crate) color: [f32; 4],
    pub(crate) emissive: [f32; 4],
    pub(crate) pbr: [f32; 4],
    pub(crate) flags: [f32; 4],
    pub(crate) maps: [f32; 4],
    pub(crate) more: [f32; 4],
    pub(crate) shade: [f32; 4],
    pub(crate) comb: [f32; 4],
    pub(crate) key: [f32; 4],
    pub(crate) params: [[f32; 4]; 8],
}

pub(crate) fn tone(c: Color, by: f32) -> [f32; 4] {
    [linear(c.0[0]) * by, linear(c.0[1]) * by, linear(c.0[2]) * by, c.0[3]]
}

fn turn(eye: &Eye, [x, y, z]: [f32; 3]) -> [f32; 3] {
    let (r, u, f) = eye.basis();
    unit3(std::array::from_fn(|i| r[i] * x - u[i] * y - f[i] * z))
}

pub(crate) fn light_from(stage: &Stage3d, eye: &Eye, lamp: &Lamp) -> Light3 {
    let local = stage.world.inverse().apply(lamp.at());
    let px = [local[0] + stage.origin[0] * stage.size[0], local[1] + stage.origin[1] * stage.size[1]];
    let at = eye.place(px, lamp.z, stage.size);
    let unit = eye.unit(stage.size[1]);
    let heading = lamp.heading();
    let dir = match lamp.beam {
        Beam::Tube => {
            let [a, b, ..] = lamp.world.0;
            turn(eye, [a, b, 0.0])
        }
        _ => turn(eye, heading),
    };
    let cos = |deg: f32| (deg * 0.5).to_radians().cos();
    let (outer, inner) = (cos(lamp.cone[1]), cos(lamp.cone[0]));
    let mut color = tone(lamp.color, lamp.intensity);
    color[3] = lamp.beam as u8 as f32;
    Light3 {
        at: [at[0], at[1], at[2], lamp.radius * unit],
        color,
        dir: [dir[0], dir[1], dir[2], outer.min(inner - 1e-4)],
        more: [inner, lamp.length * unit, 0.0, 0.0],
    }
}

pub(crate) fn scene3(
    frame: &Frame,
    (stage, eye): (&Stage3d, &Eye),
    slice: usize,
    (env, debug): (bool, u8),
    cap: usize,
) -> Scene3 {
    let (_, u, f) = eye.basis();
    let mut lights = [Light3::default(); MAX_LIGHTS];
    let lamps = frame.lamps.iter().take(cap.min(MAX_LIGHTS));
    let count = lamps.clone().count();
    lamps.zip(&mut lights).for_each(|(l, out)| *out = light_from(stage, eye, l));
    let bare = count == 0 && frame.ambient.is_none() && frame.sky.is_none() && !env;
    let ambient = frame.ambient.map_or([0.0; 4], |c| tone(c, 1.0));
    let sky = frame.sky.map_or([0.0; 4], |c| tone(c, 1.0));
    let (ambient, sky, count) = match bare {
        true => {
            let key = turn(eye, [0.45, 0.6, -0.65]);
            lights[0] = Light3 {
                at: [0.0; 4],
                color: [2.2, 2.2, 2.2, 3.0],
                dir: [-key[0], -key[1], -key[2], 0.0],
                more: [0.0; 4],
            };
            ([0.12, 0.12, 0.12, 1.0], [0.18, 0.18, 0.2, 1.0], 1)
        }
        false => ([ambient[0], ambient[1], ambient[2], 1.0], sky, count),
    };
    let [near, far] = stage.slice(slice);
    let shine = frame.quality > Quality::Q0;
    Scene3 {
        clip: eye.clip(),
        eye: [eye.at[0], eye.at[1], eye.at[2], count as f32],
        forward: [f[0], f[1], f[2], 0.0],
        ambient,
        sky,
        up: [u[0], u[1], u[2], 0.0],
        slice: [near, far, f32::from(u8::from(env)), f32::from(u8::from(shine && env))],
        wind: wind(stage, eye),
        fade: [stage.fade, f32::from(debug), 0.0, 0.0],
        lights,
    }
}

fn wind(stage: &Stage3d, eye: &Eye) -> [f32; 4] {
    let [x, y] = stage.wind;
    let (r, u, _) = eye.basis();
    let k = 0.01;
    [r[0] * x * k - u[0] * y * k, r[1] * x * k - u[1] * y * k, r[2] * x * k - u[2] * y * k, stage.time]
}
