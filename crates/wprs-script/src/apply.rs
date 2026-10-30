use crate::wire::Cursor;
use rquickjs::{Array, Ctx, Object, Result, TypedArray};
use wprs_scene::BANDS;

const MODES: [&str; 5] = ["wallpaper", "lockscreen", "screensaver", "editor", "preview"];
const CHANNELS: [&str; 3] = ["left", "right", "average"];

fn fill(array: &TypedArray<f32>, bytes: &[u8]) {
    if let Some(raw) = array.as_raw() {
        let n = raw.len().min(bytes.len());
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), raw.as_ptr().cast::<u8>(), n) };
    }
}

fn floats(list: &Array, values: &[f32]) -> Result<()> {
    values.iter().enumerate().try_for_each(|(i, v)| list.set(i, *v))
}

fn child<'js>(ctx: &Ctx<'js>, parent: &Object<'js>, key: &str) -> Result<Object<'js>> {
    match parent.get::<_, Option<Object>>(key)? {
        Some(found) => Ok(found),
        None => {
            let made = Object::new(ctx.clone())?;
            parent.set(key, made.clone())?;
            Ok(made)
        }
    }
}

fn pair<'js>(ctx: &Ctx<'js>, at: &Object<'js>, key: &str, values: &[f32]) -> Result<()> {
    let list = match at.get::<_, Option<Array>>(key)? {
        Some(list) => list,
        None => {
            let made = Array::new(ctx.clone())?;
            at.set(key, made.clone())?;
            made
        }
    };
    floats(&list, values)
}

fn reduce(bands: &[u8], n: usize) -> impl Iterator<Item = f32> + '_ {
    let value =
        |i: usize| f32::from_le_bytes([bands[4 * i], bands[4 * i + 1], bands[4 * i + 2], bands[4 * i + 3]]);
    let width = BANDS / n;
    (0..n).map(move |k| (k * width..(k + 1) * width).map(value).fold(0.0, f32::max))
}

pub(crate) fn apply<'js>(ctx: &Ctx<'js>, state: &Object<'js>, bytes: &[u8]) -> Result<Option<String>> {
    let (c, frame) = (&mut Cursor(bytes), state.get::<_, Object>("frame")?);
    frame.set("time", c.f64()?)?;
    frame.set("dt", c.f32()?)?;
    frame.set("wall", c.f64()?)?;
    let screen = [c.u32()? as f32, c.u32()? as f32];
    pair(ctx, &frame, "screen", &screen)?;
    pair(ctx, &frame, "view", &c.f32s::<4>()?)?;
    frame.set("fps", c.u32()?)?;
    frame.set("quality", c.u8()?)?;
    frame.set("mode", MODES.get(usize::from(c.u8()?)).copied().unwrap_or(MODES[0]))?;
    let cursor = child(ctx, &frame, "cursor")?;
    let (x, y, inside, buttons) = (c.f32()?, c.f32()?, c.flag()?, c.u8()?);
    cursor.set("x", x)?;
    cursor.set("y", y)?;
    cursor.set("inside", inside)?;
    cursor.set("buttons", buttons)?;
    let zone = c.opt()?.map(String::from);
    if let Some(zone) = &zone {
        frame.set("zone", zone.as_str())?;
    }
    if let Some(props) = c.opt()? {
        frame.set("props", ctx.json_parse(props)?)?;
        state.set("fresh", true)?;
    }
    let actions = Array::new(ctx.clone())?;
    for i in 0..c.u16()? {
        actions.set(i, c.str()?)?;
    }
    frame.set("actions", actions)?;
    if c.flag()? {
        let audio = child(ctx, &frame, "audio")?;
        let buffers = state.get::<_, Object>("buffers")?;
        let mut all = Vec::with_capacity(3);
        for name in CHANNELS {
            let bands = c.take(4 * BANDS)?;
            fill(&audio.get::<_, TypedArray<f32>>(name)?, bands);
            all.push(bands);
        }
        for n in [16usize, 32] {
            let Some(set) = buffers.get::<_, Option<Object>>(n as u32)? else { continue };
            for (name, bands) in CHANNELS.iter().zip(&all) {
                let reduced: Vec<u8> = reduce(bands, n).flat_map(f32::to_le_bytes).collect();
                fill(&set.get::<_, TypedArray<f32>>(*name)?, &reduced);
            }
        }
        if let Some(set) = buffers.get::<_, Option<Object>>(64u32)? {
            for (name, bands) in CHANNELS.iter().zip(&all) {
                fill(&set.get::<_, TypedArray<f32>>(*name)?, bands);
            }
        }
        for name in ["bass", "mid", "treble", "level"] {
            audio.set(name, c.f32()?)?;
        }
        audio.set("beat", c.flag()?)?;
    }
    let timelines = child(ctx, &frame, "timelines")?;
    for _ in 0..c.u16()? {
        let at = child(ctx, &timelines, c.str()?)?;
        at.set("time", c.f32()?)?;
        at.set("duration", c.f32()?)?;
        at.set("playing", c.flag()?)?;
        at.set("speed", c.f32()?)?;
    }
    let events = Array::new(ctx.clone())?;
    for i in 0..c.u16()? {
        let e = Object::new(ctx.clone())?;
        e.set("timeline", c.str()?)?;
        e.set("name", c.opt()?)?;
        e.set("t", c.f32()?)?;
        events.set(i, e)?;
    }
    frame.set("events", events)?;
    let reads = child(ctx, &frame, "reads")?;
    for _ in 0..c.u16()? {
        let at = child(ctx, &reads, c.str()?)?;
        for key in ["position", "scale", "skew", "origin", "parallax"] {
            pair(ctx, &at, key, &c.f32s::<2>()?)?;
        }
        at.set("rotation", c.f32()?)?;
        at.set("opacity", c.f32()?)?;
        at.set("visible", c.flag()?)?;
        pair(ctx, &at, "color", &c.f32s::<4>()?)?;
        match c.u8()? {
            1 => at.set("size", c.f32()?)?,
            2 => pair(ctx, &at, "size", &c.f32s::<2>()?)?,
            _ => at.remove("size")?,
        }
        at.set("text", c.opt()?)?;
    }
    let touches = Array::new(ctx.clone())?;
    for i in 0..c.u16()? {
        let t = Object::new(ctx.clone())?;
        t.set("layer", c.str()?)?;
        t.set("kind", c.u8()?)?;
        pair(ctx, &t, "local", &c.f32s::<2>()?)?;
        pair(ctx, &t, "scene", &c.f32s::<2>()?)?;
        touches.set(i, t)?;
    }
    frame.set("touches", touches)?;
    Ok(zone)
}
