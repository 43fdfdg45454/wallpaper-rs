use crate::{Draw, EffectDraw, Frame, Lamp, Look, Placed, Stage3d};

#[derive(Debug, Default)]
pub struct Spare {
    draws: Vec<Draw<'static>>,
    effects: Vec<EffectDraw<'static>>,
    lamps: Vec<Lamp<'static>>,
    stages: Vec<Stage3d<'static>>,
    placed: Vec<Placed<'static>>,
    looks: Vec<Look>,
}

pub(crate) type Lists<'a> =
    (Vec<Draw<'a>>, Vec<EffectDraw<'a>>, Vec<Lamp<'a>>, Vec<Stage3d<'a>>, Vec<Placed<'a>>, Vec<Look>);

fn emptied<T, U>(list: Vec<T>) -> Vec<U> {
    list.into_iter().filter_map(|_| None).collect()
}

impl Frame<'_> {
    pub fn spare(self) -> Spare {
        let (draws, effects, lamps) = (emptied(self.draws), emptied(self.effects), emptied(self.lamps));
        let (stages, placed, mut looks) = (emptied(self.stages), emptied(self.placed), self.looks);
        looks.clear();
        Spare { draws, effects, lamps, stages, placed, looks }
    }
}

impl Spare {
    pub(crate) fn lists<'a>(self) -> Lists<'a> {
        let (draws, effects, lamps) = (emptied(self.draws), emptied(self.effects), emptied(self.lamps));
        (draws, effects, lamps, emptied(self.stages), emptied(self.placed), self.looks)
    }
}
