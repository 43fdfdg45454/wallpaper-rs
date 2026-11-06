use crate::{Info, Target, sized};

pub fn plain(info: &Info, target: Target) -> bool {
    let target = if info.hdr { target.lit() } else { target };
    let (side, _) = target.unit();
    let (w, h) = sized(info, 0);
    info.shaped() && target.compressed() && (info.cube || w % side != 0 || h % side != 0)
}

pub fn gpu_bytes(info: &Info, target: Target, mips: bool) -> u64 {
    let shown = match (plain(info, target), info.hdr) {
        (true, true) => Target::Half,
        (true, false) => Target::Rgba,
        (false, true) => target.lit(),
        (false, false) => target,
    };
    if info.shaped() {
        return shown.bytes(info.picture(0));
    }
    let target = shown;
    let count = if mips { info.levels } else { 1 };
    (0..count).map(|l| target.bytes(sized(info, l))).sum()
}
