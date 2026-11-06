use super::super::pipes::{Mode, Shader};
use super::super::{Fill, Op, Src};
use super::gather;

fn draw(target: Src, from: Src) -> Op {
    Op::Draw { target, entry: "fs_texture", mode: Mode::Normal, slot: 0, tex: [from, Src::White, Src::Clear] }
}

fn effect(target: Src, tex: u32) -> Op {
    let shader = Shader::Fx(0, "fs_copy");
    let (slot, more, mode, load, vertices, scissor) = (0, None, Mode::Replace, false, 3, None);
    Op::Effect(Fill { target, shader, slot, tex, more, mode, load, vertices, scissor })
}

fn order(ops: &[Op]) -> Vec<(Option<Src>, &'static str)> {
    ops.iter()
        .map(|op| match op {
            Op::Clear(t, _) => (Some(*t), "clear"),
            Op::Draw { target, tex, .. } => (Some(*target), if tex[0] == Src::Pool(1) { "a" } else { "b" }),
            Op::Effect(f) => (Some(f.target), "effect"),
            _ => (None, "other"),
        })
        .collect()
}

fn sets(read: Src) -> Vec<[Src; 11]> {
    let mut set = [Src::Clear; 11];
    set[0] = read;
    vec![set]
}

#[test]
fn work_off_the_canvas_moves_ahead_so_canvas_draws_share_a_pass() {
    let canvas = Src::Canvas;
    let mut ops = [
        Op::Clear(canvas, [0.0; 4]),
        draw(canvas, Src::Pool(1)),
        effect(Src::Pool(2), 0),
        draw(canvas, Src::Pool(2)),
    ];
    gather(&mut ops, &sets(Src::Pool(0)));
    let found = order(&ops);
    assert_eq!(found.iter().map(|o| o.1).collect::<Vec<_>>(), ["effect", "clear", "a", "b"]);
}

#[test]
fn work_that_reads_the_canvas_or_overwrites_a_picture_still_in_use_stays_in_place() {
    let canvas = Src::Canvas;
    for (read, target) in [(Src::Canvas, Src::Pool(2)), (Src::Pool(0), Src::Pool(1))] {
        let mut ops = [draw(canvas, Src::Pool(1)), effect(target, 0), draw(canvas, Src::Pool(2))];
        gather(&mut ops, &sets(read));
        assert_eq!(order(&ops).iter().map(|o| o.1).collect::<Vec<_>>(), ["a", "effect", "b"]);
    }
}
