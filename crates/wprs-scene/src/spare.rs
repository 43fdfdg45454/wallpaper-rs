use crate::{Draw, EffectDraw, Frame, Lamp};

#[derive(Debug, Default)]
pub struct Spare {
    draws: Vec<Draw<'static>>,
    effects: Vec<EffectDraw<'static>>,
    lamps: Vec<Lamp<'static>>,
}

fn emptied<T, U>(list: Vec<T>) -> Vec<U> {
    list.into_iter().filter_map(|_| None).collect()
}

impl Frame<'_> {
    pub fn spare(self) -> Spare {
        Spare { draws: emptied(self.draws), effects: emptied(self.effects), lamps: emptied(self.lamps) }
    }
}

impl Spare {
    pub(crate) fn lists<'a>(self) -> (Vec<Draw<'a>>, Vec<EffectDraw<'a>>, Vec<Lamp<'a>>) {
        (emptied(self.draws), emptied(self.effects), emptied(self.lamps))
    }
}
