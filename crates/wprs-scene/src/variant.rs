use crate::{Descriptor, EffectUse, MAX_VARIANTS, Param, Value, Variant};

impl Param {
    pub fn whole(&self) -> Option<f64> {
        match self {
            Param::Number(n) => Some(f64::from(*n)),
            Param::Bool(b) => Some(f64::from(u8::from(*b))),
            _ => None,
        }
    }
}

impl Variant {
    fn at(&self, value: &Param) -> Option<usize> {
        self.values.iter().position(|v| v == value)
    }

    fn base(&self) -> usize {
        self.at(&self.default).unwrap_or(0)
    }

    fn built(&self, e: &EffectUse, pick: usize) -> usize {
        match e.params.get(&self.id) {
            Some(Value::Fixed(v)) => self.at(v).unwrap_or(self.base()),
            Some(Value::Bound(b)) => {
                let mapped = || b.map.iter().flat_map(|m| m.values()).filter_map(|v| self.at(v));
                match b.map.is_none() || mapped().any(|k| k == pick) {
                    true => pick,
                    false => mapped().min().unwrap_or(self.base()),
                }
            }
            None => pick,
        }
    }

    fn on(&self) -> usize {
        self.at(&Param::Bool(true)).or_else(|| self.at(&Param::Number(1.0))).unwrap_or(self.values.len() - 1)
    }
}

impl Descriptor {
    pub fn variant<'a>(&'a self, e: &EffectUse, value: impl Fn(&'a str) -> Option<Param>) -> u32 {
        let filled = |id: &'a str| {
            let layered = e.params.get(id).and_then(Value::binding).is_some_and(|b| b.layer.is_some());
            layered
                || match value(id) {
                    Some(Param::Text(t)) => !t.is_empty(),
                    Some(_) => false,
                    None => self.painted(id),
                }
        };
        self.options.iter().enumerate().fold(0, |code, (k, o)| {
            let pick = match value(&o.id).and_then(|v| o.at(&v)) {
                Some(k) => k,
                None if o.when_bound.as_deref().is_some_and(filled) => o.on(),
                None => o.base(),
            };
            code | ((o.built(e, pick) as u32 & 15) << (4 * k))
        })
    }

    fn painted(&self, id: &str) -> bool {
        self.param(id).is_some_and(|i| matches!(&self.params[i].default, Param::Text(t) if !t.is_empty()))
    }

    pub fn constants(&self, variant: u32) -> Vec<(String, f64)> {
        let pick =
            |k: usize, o: &Variant| o.values.get((variant >> (4 * k) & 15) as usize).and_then(Param::whole);
        let found = |(k, o): (usize, &Variant)| Some((o.id.clone(), pick(k, o).or(o.default.whole())?));
        self.options.iter().enumerate().filter_map(found).collect()
    }

    pub fn choices(&self, e: &EffectUse) -> Vec<Vec<usize>> {
        let picks = self.options.iter().map(|o| match e.params.get(&o.id) {
            Some(Value::Fixed(v)) => vec![o.at(v).unwrap_or(o.base())],
            Some(Value::Bound(b)) => match &b.map {
                Some(map) => map.values().filter_map(|v| o.at(v)).collect(),
                None => (0..o.values.len()).collect(),
            },
            None => match o.when_bound.as_deref().map(|t| (t, e.params.get(t))) {
                Some((_, Some(Value::Fixed(Param::Text(t))))) if !t.is_empty() => vec![o.on()],
                Some((_, Some(Value::Bound(b)))) if b.layer.is_some() => vec![o.on()],
                Some((_, Some(Value::Bound(_)))) => vec![o.base(), o.on()],
                Some((t, None)) if self.painted(t) => vec![o.on()],
                _ => vec![o.base()],
            },
        });
        let tidy = |(o, mut c): (&Variant, Vec<usize>)| {
            c.sort_unstable();
            c.dedup();
            if c.is_empty() {
                c.push(o.base());
            }
            c
        };
        self.options.iter().zip(picks).map(tidy).collect()
    }

    pub fn combinations(&self, e: &EffectUse) -> usize {
        self.choices(e).iter().fold(1, |n, c| n.saturating_mul(c.len()))
    }

    pub fn variants(&self, e: &EffectUse) -> Result<Vec<u32>, String> {
        let count = self.combinations(e);
        if count > MAX_VARIANTS {
            return Err(format!("needs {count} variants, more than {MAX_VARIANTS}"));
        }
        let mut codes = vec![0];
        for (k, c) in self.choices(e).iter().enumerate() {
            codes = codes
                .iter()
                .flat_map(|code| c.iter().map(move |p| code | ((*p as u32 & 15) << (4 * k))))
                .collect();
        }
        codes.sort_unstable();
        codes.dedup();
        Ok(codes)
    }
}
