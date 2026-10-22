use std::collections::BTreeMap;

fn kept(ids: &[String], base: &BTreeMap<String, f64>) -> Vec<bool> {
    let known: Vec<(usize, f64)> =
        ids.iter().enumerate().filter_map(|(i, id)| Some((i, *base.get(id)?))).collect();
    let (mut tails, mut prev): (Vec<usize>, Vec<Option<usize>>) = (Vec::new(), vec![None; known.len()]);
    for (k, (_, pos)) in known.iter().enumerate() {
        let at = tails.partition_point(|&t| known[t].1 < *pos);
        prev[k] = at.checked_sub(1).map(|a| tails[a]);
        if at == tails.len() { tails.push(k) } else { tails[at] = k }
    }
    let mut keep = vec![false; ids.len()];
    let mut at = tails.last().copied();
    while let Some(k) = at {
        keep[known[k].0] = true;
        at = prev[k];
    }
    keep
}

pub fn positions(ids: &[String], base: &BTreeMap<String, f64>) -> Vec<f64> {
    let keep = kept(ids, base);
    let mut out: Vec<Option<f64>> = ids.iter().zip(&keep).map(|(id, k)| k.then(|| base[id])).collect();
    let mut start = 0;
    while start < out.len() {
        if out[start].is_some() {
            start += 1;
            continue;
        }
        let end = (start..out.len()).find(|&i| out[i].is_some()).unwrap_or(out.len());
        let (low, high) = (start.checked_sub(1).and_then(|i| out[i]), out.get(end).copied().flatten());
        let count = (end - start) as f64;
        for (n, slot) in out[start..end].iter_mut().enumerate() {
            let step = n as f64 + 1.0;
            *slot = Some(match (low, high) {
                (Some(l), Some(h)) => l + (h - l) * step / (count + 1.0),
                (Some(l), None) => l + step,
                (None, Some(h)) => h - (count + 1.0 - step),
                (None, None) => n as f64,
            });
        }
        start = end;
    }
    out.into_iter().flatten().collect()
}
