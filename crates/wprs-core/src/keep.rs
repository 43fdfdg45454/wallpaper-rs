use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

pub(crate) fn keep(old: &str, fresh: &str) -> Option<String> {
    let (mut doc, new) = (old.parse::<DocumentMut>().ok()?, fresh.parse::<DocumentMut>().ok()?);
    let mut last = 0;
    visit(doc.as_item_mut(), &mut |t| last = last.max(t.position().unwrap_or(0)));
    table(doc.as_table_mut(), new.as_table(), &mut last);
    Some(doc.to_string().trim_start_matches('\n').to_owned())
}

fn table(old: &mut dyn TableLike, new: &dyn TableLike, last: &mut isize) {
    let gone: Vec<String> = old.iter().map(|(k, _)| k.to_owned()).filter(|k| !new.contains_key(k)).collect();
    gone.iter().for_each(|key| drop(old.remove(key)));
    for (key, item) in new.iter() {
        match old.get_mut(key) {
            Some(slot) => merge(slot, item, last),
            None => drop(old.insert(key, fresh(item, last))),
        }
    }
}

fn merge(old: &mut Item, new: &Item, last: &mut isize) {
    if let (Some(o), Some(n)) = (old.as_table_like_mut(), new.as_table_like()) {
        return table(o, n, last);
    }
    match (old, new) {
        (Item::Value(o), Item::Value(n)) if same(o, n) => {}
        (Item::Value(o), Item::Value(n)) => {
            let decor = o.decor().clone();
            *o = n.clone();
            *o.decor_mut() = decor;
        }
        (Item::ArrayOfTables(o), Item::ArrayOfTables(n)) if o.len() == n.len() => {
            o.iter_mut().zip(n.iter()).for_each(|(o, n)| table(o, n, last))
        }
        (o, n) => *o = fresh(n, last),
    }
}

fn fresh(item: &Item, last: &mut isize) -> Item {
    let mut item = item.clone();
    visit(&mut item, &mut |t| {
        *last += 1;
        t.set_position(Some(*last));
        t.set_implicit(true);
        t.decor_mut().set_prefix("\n");
    });
    item
}

fn visit(item: &mut Item, each: &mut dyn FnMut(&mut Table)) {
    let tables: Vec<&mut Table> = match item {
        Item::Table(t) => vec![t],
        Item::ArrayOfTables(a) => a.iter_mut().collect(),
        _ => Vec::new(),
    };
    for t in tables {
        each(t);
        t.iter_mut().for_each(|(_, item)| visit(item, each));
    }
}

fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(a), Value::String(b)) => a.value() == b.value(),
        (Value::Integer(a), Value::Integer(b)) => a.value() == b.value(),
        (Value::Float(a), Value::Float(b)) => a.value() == b.value(),
        (Value::Boolean(a), Value::Boolean(b)) => a.value() == b.value(),
        (Value::Datetime(a), Value::Datetime(b)) => a.value() == b.value(),
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(a, b)),
        (Value::InlineTable(a), Value::InlineTable(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| same(v, w)))
        }
        _ => false,
    }
}
