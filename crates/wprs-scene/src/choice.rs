use crate::{Budget, Quality, Runtime, Why};
use wprs_core::QualityChoice;

impl Quality {
    pub fn from_level(level: u8) -> Quality {
        [Quality::Q0, Quality::Q1, Quality::Q2, Quality::Q3][usize::from(level.min(QualityChoice::TOP))]
    }
}

impl Budget {
    pub fn level(&self, runtime: &Runtime) -> (Quality, Why) {
        self.choose(self.auto(runtime))
    }

    pub fn choose(&self, (auto, why): (Quality, Why)) -> (Quality, Why) {
        match self.choice.fixed() {
            Some(level) => (Quality::from_level(level), Why::User),
            None if auto < self.floor() => (self.floor(), Why::User),
            None => (auto, why),
        }
    }

    pub fn floor(&self) -> Quality {
        Quality::from_level(self.choice.floor())
    }

    pub fn over(&self, runtime: &Runtime, quality: Quality) -> Option<(f32, f32)> {
        let (cost, budget) = (runtime.cost(quality), self.tier.budget());
        (!self.choice.is_auto() && quality > Quality::Q0 && cost > budget).then_some((cost, budget))
    }
}
