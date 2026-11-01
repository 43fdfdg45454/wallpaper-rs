use super::doc::{Gltf, Node};
use super::math::{Mat, Trs, decompose, normalized};
use super::{Joint, MAX_RIG_NODES, Rigged};

pub(super) fn column(m: &[f32]) -> Mat {
    std::array::from_fn(|k| m[(k % 4) * 4 + k / 4])
}

fn rest(n: &Node) -> Result<Trs, String> {
    let trs = match n.matrix {
        Some(m) => decompose(&column(&m)),
        None => Trs {
            t: n.translation.unwrap_or([0.0; 3]),
            r: normalized(n.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0])),
            s: n.scale.unwrap_or([1.0; 3]),
        },
    };
    let finite = trs.t.iter().chain(&trs.r).chain(&trs.s).all(|v| v.is_finite());
    finite.then_some(trs).ok_or_else(|| "has a node transform that is not a number".into())
}

pub(super) fn joints(doc: &Gltf, rig: &mut Rigged) -> Result<(), String> {
    let count = doc.nodes.len();
    if count > MAX_RIG_NODES {
        return Err(format!("has more than {MAX_RIG_NODES} nodes"));
    }
    let mut parents: Vec<Option<u16>> = vec![None; count];
    for (i, n) in doc.nodes.iter().enumerate() {
        for &c in &n.children {
            if c >= count {
                return Err(format!("node {i} names the missing child {c}"));
            }
            if c == i || parents[c].is_some() {
                return Err("has a bone cycle or a node with two parents".into());
            }
            parents[c] = Some(i as u16);
        }
    }
    for (i, n) in doc.nodes.iter().enumerate() {
        let name = n.name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("node_{i}"));
        let point = n.extras["point"] == true;
        rig.joints.push(Joint { name, parent: parents[i], rest: rest(n)?, point });
    }
    let mut order: Vec<u16> = (0..count).filter(|i| parents[*i].is_none()).map(|i| i as u16).collect();
    let mut at = 0;
    while let Some(&node) = order.get(at) {
        order.extend(doc.nodes[node as usize].children.iter().map(|c| *c as u16));
        at += 1;
    }
    if order.len() != count {
        return Err("has a bone cycle".into());
    }
    rig.order = order;
    Ok(())
}
