use crate::{Doc, Hlc, View};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Winner {
    Mine,
    Theirs,
}

pub type Clash = fn(Option<&View>, Option<&View>, Option<&View>) -> Option<Winner>;

#[derive(Clone, Copy)]
pub struct Rules {
    pub weak: fn(&str) -> bool,
    pub keep_absent: bool,
    pub needs_strong: bool,
    pub clash: Clash,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { weak: |_| false, keep_absent: false, needs_strong: false, clash: |_, _, _| None }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Step {
    pub push: Option<(i64, Doc)>,
    pub apply: Option<Option<View>>,
    pub base: Option<(i64, Doc)>,
    pub copy: Option<View>,
}

fn after(now: &Hlc, seen: Hlc) -> Hlc {
    if *now > seen { now.clone() } else { Hlc(seen.0, seen.1 + 1, now.2.clone()) }
}

pub fn step(
    base: Option<&(i64, Doc)>,
    remote: Option<&(i64, Doc)>,
    local: Option<&View>,
    rules: Rules,
    now: &Hlc,
) -> Step {
    let base_doc = base.map(|(_, doc)| doc);
    let mut local = local.cloned();
    local.iter_mut().for_each(|view| view.retain(|_, value| !value.is_null()));
    let local = local.as_ref();
    let unknown = rules.keep_absent && local.is_none();
    let only_weak = local.is_some_and(|view| view.keys().all(|key| (rules.weak)(key)));
    let silent = unknown || (rules.needs_strong && base.is_none() && only_weak);
    let stamped = if silent { None } else { Doc::stamp(base_doc, local, rules.weak, now) };
    let mine = stamped.clone().or_else(|| base_doc.cloned());
    let (merged, push, base, copy) = match remote {
        Some((seq, theirs)) => {
            let stale = base_doc.is_some_and(|b| theirs.clock() < b.clock());
            let against = if stale { base_doc.unwrap_or(theirs) } else { theirs };
            let seen = base_doc.filter(|b| stamped.is_some() && *b != against).map(Doc::view);
            let clash = seen.and_then(|seen| (rules.clash)(seen.as_ref(), local, against.view().as_ref()));
            let (merged, copy) = match clash {
                Some(Winner::Theirs) => (against.clone(), local.cloned()),
                Some(Winner::Mine) => {
                    let at = after(now, against.clock());
                    let restamped = Doc::stamp(Some(against), local, |_| false, &at);
                    (restamped.unwrap_or_else(|| against.clone()), against.view())
                }
                None => (mine.map_or_else(|| against.clone(), |m| m.merge(against)), None),
            };
            let push = (merged != *theirs).then(|| (*seq, merged.clone()));
            (Some(merged), push, Some((*seq, against.clone())), copy)
        }
        None => (mine, stamped.map(|doc| (base.map_or(0, |b| b.0), doc)), base.cloned(), None),
    };
    let view = merged.as_ref().and_then(Doc::view);
    let apply = (!unknown && merged.is_some() && view.as_ref() != local).then_some(view);
    Step { push, apply, base, copy }
}
