use crate::{Draw, EffectDraw, Frame};

#[derive(Debug, Default)]
pub struct Spare {
    draws: Vec<Draw<'static>>,
    effects: Vec<EffectDraw<'static>>,
}

fn emptied<T, U>(list: Vec<T>) -> Vec<U> {
    list.into_iter().filter_map(|_| None).collect()
}

impl Frame<'_> {
    pub fn spare(self) -> Spare {
        Spare { draws: emptied(self.draws), effects: emptied(self.effects) }
    }
}

impl Spare {
    pub(crate) fn lists<'a>(self) -> (Vec<Draw<'a>>, Vec<EffectDraw<'a>>) {
        (emptied(self.draws), emptied(self.effects))
    }
}
