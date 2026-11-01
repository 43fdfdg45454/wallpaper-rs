use super::Skin;

impl Skin {
    pub fn bind(&mut self, all: bool) {
        if self.bones.is_empty() {
            return;
        }
        let worlds = self.worlds();
        let heads: Vec<[f32; 2]> =
            self.bones.iter().map(|b| self.head(&worlds, *b)).map(|h| [h[0], h[1]]).collect();
        let index = |n: usize| self.bones.iter().position(|b| *b == n);
        let parents: Vec<Option<usize>> = self.bones.iter().map(|b| self.up(*b).and_then(index)).collect();
        let chosen: Vec<usize> =
            (0..self.points.len()).filter(|v| all || self.loose[*v] || self.weights[*v].is_empty()).collect();
        let points: Vec<[f32; 2]> = chosen.iter().map(|v| [self.points[*v][0], self.points[*v][1]]).collect();
        let band = (self.size[0].max(self.size[1]) * 0.04).max(1.0);
        let found = wprs_scene::nearest_bones(&points, &heads, &parents, band);
        for (v, w) in chosen.into_iter().zip(found) {
            self.weights[v] =
                w.iter().filter(|x| x.1 > 0.0).map(|(b, x)| (self.bones[*b as usize], *x)).collect();
            self.loose[v] = false;
        }
    }

    pub(super) fn settle(&mut self) {
        let all = self.auto;
        self.bind(all);
    }

    pub fn raise(&mut self, stroke: &[[f32; 2]], radius: f32, by: f32) {
        let r = radius.max(1e-3);
        for p in &mut self.points {
            let near = stroke.iter().map(|s| (s[0] - p[0]).hypot(s[1] - p[1]) / r).fold(1.0, f32::min);
            let z = p[2] + by * (1.0 - near * near).powi(2);
            p[2] = z.clamp(-wprs_scene::MAX_SKETCH_DEPTH, wprs_scene::MAX_SKETCH_DEPTH);
        }
    }

    pub fn weight(&self, vertex: usize, bone: usize) -> f32 {
        self.weights[vertex].iter().filter(|w| w.0 == bone).map(|w| w.1).sum()
    }
}
