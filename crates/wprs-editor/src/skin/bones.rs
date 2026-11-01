use super::Skin;
use super::glb::items;
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use wprs_core::message::said;
use wprs_scene::mesh_math::{Mat, decompose, invert, mul};

fn translated(m: &Mat, [x, y]: [f32; 2]) -> Mat {
    let mut out = *m;
    (out[3], out[7]) = (x, y);
    out
}

impl Skin {
    fn local(&self, worlds: &[Mat], parent: Option<usize>, world: &Mat) -> wprs_scene::Trs {
        decompose(&parent.map_or(*world, |p| mul(&invert(&worlds[p]), world)))
    }

    pub fn named(&self, name: &str) -> Result<()> {
        let fits = !name.is_empty() && name.len() <= 64 && !name.contains(['/', '#']);
        ensure!(
            fits,
            said("editor.bone_name", "a bone has a name of 1 to 64 characters without / or #", &[])
        );
        let free = !self.names.iter().any(|n| n == name);
        ensure!(free, said("editor.bone_taken", "another bone or part is already called \"%1\"", &[&name]));
        Ok(())
    }

    pub fn add_bone(&mut self, name: &str, parent: Option<usize>, at: [f32; 2]) -> Result<usize> {
        self.named(name)?;
        let worlds = self.worlds();
        let world = translated(&wprs_scene::mesh_math::IDENTITY, at);
        let local = self.local(&worlds, parent, &world);
        let nodes =
            self.glb.json["nodes"].as_array_mut().ok_or_else(|| anyhow!("the mesh file has no nodes"))?;
        nodes.push(json!({}));
        self.names.push(name.to_owned());
        self.parents.push(parent);
        self.locals.push(local);
        self.bones.push(self.names.len() - 1);
        Ok(self.names.len() - 1)
    }

    pub fn place(&mut self, node: usize, at: [f32; 2]) {
        let worlds = self.worlds();
        let world = translated(&worlds[node], at);
        self.locals[node] = self.local(&worlds, self.parents[node], &world);
        let back = invert(&world);
        for (c, w) in worlds.iter().enumerate() {
            if self.parents[c] == Some(node) {
                self.locals[c] = decompose(&mul(&back, w));
            }
        }
    }

    pub fn hang(&mut self, node: usize, parent: Option<usize>) -> Result<()> {
        let below = |p: usize| std::iter::successors(Some(p), |q| self.parents[*q]).any(|q| q == node);
        ensure!(
            parent.is_none_or(|p| !below(p)),
            said("editor.bone_loop", "a bone cannot hang from itself or its children", &[])
        );
        let worlds = self.worlds();
        self.locals[node] = self.local(&worlds, parent, &worlds[node]);
        self.parents[node] = parent;
        Ok(())
    }

    pub fn remove_bone(&mut self, node: usize) -> Result<()> {
        let part = self.glb.json["nodes"][node].get("mesh").is_some();
        ensure!(!part, said("editor.bone_part", "the bone carries a part of the mesh and stays", &[]));
        let up = self.parents[node];
        let children: Vec<usize> = (0..self.names.len()).filter(|c| self.parents[*c] == Some(node)).collect();
        for c in children {
            self.hang(c, up)?;
        }
        let heir = self.up(node);
        for (w, loose) in self.weights.iter_mut().zip(&mut self.loose) {
            let gone: f32 = w.iter().filter(|x| x.0 == node).map(|x| x.1).sum();
            w.retain(|x| x.0 != node);
            match heir {
                Some(h) if gone > 0.0 => match w.iter_mut().find(|x| x.0 == h) {
                    Some(x) => x.1 += gone,
                    None => w.push((h, gone)),
                },
                _ => {}
            }
            *loose |= w.is_empty();
        }
        let shift = |n: usize| if n > node { n - 1 } else { n };
        self.names.remove(node);
        self.locals.remove(node);
        self.parents.remove(node);
        self.parents.iter_mut().for_each(|p| *p = p.map(shift));
        self.bones.retain(|b| *b != node);
        self.bones.iter_mut().for_each(|b| *b = shift(*b));
        self.weights.iter_mut().flatten().for_each(|w| w.0 = shift(w.0));
        self.pieces.iter_mut().for_each(|p| p.node = shift(p.node));
        let doc = &mut self.glb.json;
        if let Some(nodes) = doc["nodes"].as_array_mut() {
            nodes.remove(node);
        }
        let renumber = |list: Option<&mut Value>| {
            if let Some(items) = list.and_then(Value::as_array_mut) {
                items.retain(|v| v.as_u64() != Some(node as u64));
                items.iter_mut().for_each(|v| *v = json!(shift(v.as_u64().unwrap_or(0) as usize)));
            }
        };
        items(doc, "nodes").for_each(|n| renumber(n.get_mut("children")));
        items(doc, "scenes").for_each(|s| renumber(s.get_mut("nodes")));
        for a in items(doc, "animations") {
            let Some(channels) = a.get_mut("channels").and_then(Value::as_array_mut) else { continue };
            channels.retain(|c| c["target"]["node"].as_u64() != Some(node as u64));
            for c in channels.iter_mut() {
                if let Some(n) = c["target"]["node"].as_u64() {
                    c["target"]["node"] = json!(shift(n as usize));
                }
            }
        }
        Ok(())
    }
}
