use super::expr::{CMP, MAX_LEN, Parser, tokens};
use super::value::same;
use super::{Condition, Control, Expr, Property, Values};
use serde_json::{Value as Json, json};
use std::cmp::Ordering;
use std::collections::BTreeMap;

type Props = BTreeMap<String, Property>;

fn ty(value: &Json) -> Option<&'static str> {
    match value {
        Json::Bool(_) => Some("true or false"),
        Json::Number(_) => Some("a number"),
        Json::String(_) => Some("a text"),
        _ => None,
    }
}

impl Property {
    fn sort(&self) -> Option<&'static str> {
        match &self.control {
            Control::Bool { .. } => Some("true or false"),
            Control::Slider(_) => Some("a number"),
            Control::Combo { default, .. } => ty(default),
            Control::Color { .. } | Control::Text { .. } | Control::File { .. } | Control::Folder { .. } => {
                Some("a text")
            }
            _ => None,
        }
    }
}

impl Expr {
    pub fn parse(text: &str) -> Result<Expr, String> {
        if text.chars().count() > MAX_LEN {
            return Err(format!("has more than {MAX_LEN} characters"));
        }
        let mut parser = Parser::new(tokens(text)?);
        let found = parser.either()?;
        parser.done().then_some(found).ok_or_else(|| "has something left after the condition".into())
    }

    pub fn ids(&self) -> Vec<String> {
        match self {
            Expr::Id(id) => vec![id.clone()],
            Expr::Lit(_) => Vec::new(),
            Expr::Not(e) => e.ids(),
            Expr::Both(a, b) | Expr::Either(a, b) | Expr::Cmp(a, _, b) => [a.ids(), b.ids()].concat(),
        }
    }

    fn sort(&self, props: &Props) -> Result<&'static str, String> {
        let truth = |e: &Expr| match e.sort(props)? {
            "true or false" => Ok("true or false"),
            other => Err(format!("!, && and || take true or false, not {other}")),
        };
        match self {
            Expr::Id(id) => {
                let prop = props.get(id).ok_or_else(|| format!("names the unknown property {id}"))?;
                prop.sort().ok_or_else(|| format!("names {id}, which has no value"))
            }
            Expr::Lit(value) => ty(value).ok_or_else(|| "has an unknown value".into()),
            Expr::Not(e) => truth(e),
            Expr::Both(a, b) | Expr::Either(a, b) => truth(a).and(truth(b)),
            Expr::Cmp(a, op, b) => match (a.sort(props)?, b.sort(props)?) {
                (x, y) if x != y => Err(format!("compares {x} with {y}")),
                (x, _) if CMP[2..].contains(op) && x != "a number" => {
                    Err(format!("{op} compares numbers only"))
                }
                _ => Ok("true or false"),
            },
        }
    }

    pub fn eval(&self, values: &Values) -> Json {
        let truth = |e: &Expr| e.eval(values) == json!(true);
        match self {
            Expr::Id(id) => values.get(id).cloned().unwrap_or(Json::Null),
            Expr::Lit(value) => value.clone(),
            Expr::Not(e) => json!(!truth(e)),
            Expr::Both(a, b) => json!(truth(a) && truth(b)),
            Expr::Either(a, b) => json!(truth(a) || truth(b)),
            Expr::Cmp(a, op, b) => {
                let (x, y) = (a.eval(values), b.eval(values));
                let order = x.as_f64().zip(y.as_f64()).and_then(|(x, y)| x.partial_cmp(&y));
                json!(match *op {
                    "==" => same(&x, &y),
                    "!=" => !same(&x, &y),
                    "<" => order == Some(Ordering::Less),
                    ">" => order == Some(Ordering::Greater),
                    "<=" => matches!(order, Some(Ordering::Less | Ordering::Equal)),
                    _ => matches!(order, Some(Ordering::Greater | Ordering::Equal)),
                })
            }
        }
    }
}

impl Condition {
    pub fn is_empty(&self) -> bool {
        matches!(self, Condition::Equal(map) if map.is_empty())
    }

    pub fn ids(&self) -> Vec<String> {
        match self {
            Condition::Equal(map) => map.keys().cloned().collect(),
            Condition::Expr(text) => Expr::parse(text).map(|e| e.ids()).unwrap_or_default(),
        }
    }

    pub fn holds(&self, props: &Props, values: &Values) -> bool {
        let equal = |(id, want): (&String, &Json)| {
            let want = props.get(id).and_then(|p| p.value(want).ok());
            want.zip(values.get(id)).is_some_and(|(w, have)| same(&w, have))
        };
        match self {
            Condition::Equal(map) => map.iter().all(equal),
            Condition::Expr(text) => Expr::parse(text).is_ok_and(|e| e.eval(values) == json!(true)),
        }
    }

    pub fn check(&self, props: &Props, id: &str) -> Result<(), String> {
        let other = |name: &String| props.get(name).filter(|_| name != id);
        match self {
            Condition::Equal(map) => map.iter().try_for_each(|(name, want)| {
                let target = other(name).ok_or_else(|| format!("depends on the unknown property {name}"))?;
                target.value(want).map(drop).map_err(|e| format!("depends on {name}, which {e}"))
            }),
            Condition::Expr(text) => {
                let expr = Expr::parse(text)?;
                if let Some(name) = expr.ids().iter().find(|n| other(n).is_none()) {
                    return Err(format!("depends on the unknown property {name}"));
                }
                match expr.sort(props)? {
                    "true or false" => Ok(()),
                    other => Err(format!("is {other}, not true or false")),
                }
            }
        }
    }
}
