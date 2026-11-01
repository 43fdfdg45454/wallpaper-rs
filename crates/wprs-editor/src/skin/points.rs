use super::Skin;
use anyhow::{Result, anyhow};
use serde_json::json;
use wprs_scene::mesh_math::{IDENTITY, decompose, invert, mul};

impl Skin {
    pub fn is_point(&self, node: usize) -> bool {
        self.glb.json["nodes"][node]["extras"]["point"] == true
    }

    pub fn lead(&self, [x, y]: [f32; 2]) -> Option<usize> {
        let far = |p: &[f32; 3]| (p[0] - x).hypot(p[1] - y);
        let near =
            (0..self.points.len()).min_by(|a, b| far(&self.points[*a]).total_cmp(&far(&self.points[*b])));
        let heaviest = near.and_then(|v| self.weights[v].iter().max_by(|a, b| a.1.total_cmp(&b.1)));
        let worlds = self.worlds();
        let head = |b: &usize| far(&self.head(&worlds, *b));
        heaviest
            .map(|w| w.0)
            .or_else(|| self.bones.iter().copied().min_by(|a, b| head(a).total_cmp(&head(b))))
    }

    pub fn add_point(&mut self, name: &str, at: [f32; 2]) -> Result<usize> {
        self.named(name)?;
        let parent = self.lead(at);
        let worlds = self.worlds();
        let mut world = IDENTITY;
        (world[3], world[7]) = (at[0], at[1]);
        let local = decompose(&parent.map_or(world, |p| mul(&invert(&worlds[p]), &world)));
        let nodes =
            self.glb.json["nodes"].as_array_mut().ok_or_else(|| anyhow!("the mesh file has no nodes"))?;
        nodes.push(json!({ "extras": { "point": true } }));
        self.names.push(name.to_owned());
        self.parents.push(parent);
        self.locals.push(local);
        Ok(self.names.len() - 1)
    }
}
