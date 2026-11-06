pub fn toggled(list: &str, item: &str, order: &str) -> String {
    let item = item.trim();
    let split = |text: &str| {
        text.split(',').map(str::trim).filter(|part| !part.is_empty()).map(String::from).collect::<Vec<_>>()
    };
    let (mut chosen, order) = (split(list), split(order));
    match chosen.iter().position(|part| part == item) {
        Some(at) => drop(chosen.remove(at)),
        None => chosen.push(item.to_owned()),
    }
    chosen.retain(|part| !part.is_empty());
    let rank = |part: &String| order.iter().position(|known| known == part).unwrap_or(order.len());
    chosen.sort_by_key(rank);
    chosen.join(",")
}

pub fn flipped(bits: &str, index: usize) -> String {
    let flip = |(at, bit): (usize, char)| if at == index { if bit == '1' { '0' } else { '1' } } else { bit };
    bits.chars().enumerate().map(flip).collect()
}
