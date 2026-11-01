use super::Swarms;
use super::maps::Asked;
use crate::Renderer;
use wprs_scene::Affine;

impl Swarms {
    pub(super) fn wall(&mut self, r: &Renderer, i: usize, shot: Asked) {
        let m = &self.maps[i];
        let rgba = super::texels::alphas(&m.texels, shot);
        if rgba.is_empty() {
            return;
        }
        let (w, h) = (shot.size.0 + 2, shot.size.1 + 2);
        let mut padded = vec![0u8; (w * h * 4) as usize];
        for (y, row) in rgba.chunks_exact(shot.size.0 as usize * 4).enumerate() {
            let at = ((y + 1) * w as usize + 1) * 4;
            padded[at..at + row.len()].copy_from_slice(row);
        }
        let (cells, d) = wprs_scene::distance_field(w, h, &padded, m.threshold, [w as f32, h as f32]);
        let bake = &mut self.herds[m.herd].bake;
        let wall = &mut bake.walls[m.part][m.emitter];
        let start = wall.start as usize;
        let n = d.len().min(wprs_scene::MAX_FIELD);
        bake.points[start..start + n].iter_mut().zip(&d).for_each(|(p, d)| *p = [*d, 0.0]);
        (wall.cells, wall.map) = ([cells[0] as u32, cells[1] as u32], [w as f32, h as f32]);
        (wall.place, wall.ready) = (Affine::translate([1.0, 1.0]).then(shot.place.inverse()).0, true);
        if let Some(Some(f)) = self.flights.get(m.herd) {
            let at = (start * 8) as u64;
            r.queue.write_buffer(&f.points, at, bytemuck::cast_slice(&bake.points[start..start + n]));
        }
    }
}
