use super::paint::Painter;
use super::pool::{BUDGET, HELD, Tex, bytes};
use std::collections::{BTreeMap, HashMap, HashSet};
use wprs_scene::{EffectDraw, Layer, Scene, Spend, Tier, over};

#[derive(Default)]
pub(crate) struct Ledger {
    spends: Vec<Spend>,
    tier: Option<Tier>,
    base: HashSet<usize>,
    sized: Vec<usize>,
    out: Vec<(String, usize)>,
    seen: (u32, u32),
    cut: HashMap<String, Vec<usize>>,
}

fn index(layer: &Layer, e: &EffectDraw<'_>) -> usize {
    layer.effects.iter().position(|u| std::ptr::eq(u, e.effect)).unwrap_or(usize::MAX)
}

impl Ledger {
    pub(crate) fn new(
        scene: &Scene,
        tier: Tier,
        refused: &mut BTreeMap<String, String>,
    ) -> (Ledger, Option<Scene>) {
        let spends = scene.spends();
        let [w, h] = scene.canvas.size.map(|v| v as f32);
        let found = over(&spends, |_| false, tier, [0.0, 0.0, w, h], 0.0);
        let built = (!found.is_empty()).then(|| scene.without(found.iter().map(|(i, _)| &spends[*i])));
        let base = found.iter().map(|(i, _)| *i).collect();
        found.into_iter().for_each(|(i, why)| drop(refused.insert(spends[i].key(), why)));
        let mut ledger = Ledger { spends, tier: Some(tier), base, ..Ledger::default() };
        ledger.mark();
        (ledger, built)
    }

    fn mark(&mut self) {
        self.cut.clear();
        let spent =
            self.base.iter().chain(&self.sized).map(|&i| (&self.spends[i].layer, self.spends[i].index));
        for (layer, at) in spent.chain(self.out.iter().map(|(l, at)| (l, *at))) {
            self.cut.entry(layer.clone()).or_default().push(at);
        }
    }

    fn key(&self, i: usize) -> String {
        self.spends[i].key()
    }
}

impl Painter<'_, '_, '_> {
    pub(crate) fn spend(&mut self) {
        let size = self.full.size();
        let held = self.caches.values().map(|c| c.tex.bytes());
        let held = held.chain(self.states.values().flat_map(|s| s.tex.iter().map(Tex::bytes))).sum::<u64>();
        self.pool.outside = held + 2 * bytes(size, self.format);
        self.pool.lend = BUDGET.max(4 * bytes(size, self.format));
        self.pool.cap = HELD.max(self.pool.outside + self.pool.lend + 2 * bytes(size, self.format));
        let ledger = &mut self.pool.ledger;
        let Some(tier) = ledger.tier.filter(|_| ledger.seen != size) else { return };
        ledger.seen = size;
        let pixels = size.0 as f32 * size.1 as f32;
        let base = &ledger.base;
        let found = over(&ledger.spends, |i| base.contains(&i), tier, self.frame.view, pixels);
        let old = std::mem::replace(&mut ledger.sized, found.iter().map(|(i, _)| *i).collect());
        ledger.mark();
        let kept: Vec<String> = ledger.base.iter().chain(&ledger.sized).map(|&i| ledger.key(i)).collect();
        old.into_iter().map(|i| ledger.key(i)).filter(|k| !kept.contains(k)).for_each(|k| {
            self.refused.remove(&k);
        });
        found.into_iter().for_each(|(i, why)| drop(self.refused.insert(ledger.key(i), why)));
    }

    pub(crate) fn cut(&self, i: usize, e: &EffectDraw<'_>) -> bool {
        let cut = &self.pool.ledger.cut;
        let layer = self.frame.draws[i].layer;
        !cut.is_empty() && cut.get(layer.id.as_str()).is_some_and(|at| at.contains(&index(layer, e)))
    }

    pub(crate) fn idle(&self, i: usize) -> bool {
        let frame = self.frame;
        let list = &frame.effects[frame.draws[i].effects.clone()];
        !self.pool.ledger.cut.is_empty() && list.iter().all(|e| self.cut(i, e))
    }

    pub(crate) fn leave(&mut self, i: usize, e: &EffectDraw<'_>, why: String) {
        let layer = self.frame.draws[i].layer;
        self.refused.insert(format!("{} on {}", e.effect.effect, layer.id), why);
        self.pool.ledger.out.push((layer.id.clone(), index(layer, e)));
        self.pool.ledger.mark();
    }
}
