use crate::draw::{TARGET_FORMAT, Target};
use crate::readback::padded_row_bytes;
use crate::{Layer, RenderError, Renderer};
use wprs_core::{Ages, Region};

impl Renderer {
    pub fn render_damaged<R>(
        &self,
        target: &mut Target,
        layers: &[Layer<'_>],
        ages: &Ages,
        read: impl FnOnce(u32, u32, u32, Option<Region>, &[u8]) -> R,
    ) -> Result<R, RenderError> {
        let size = target.size();
        let damage = ages.since(target.drawn).map(|r| r.within(size)).filter(|r| !r.covers(size));
        target.drawn = 0;
        let rendered = self.scoped(|| {
            let mut encoder =
                self.encode(&target.view, &mut target.slots, layers, size, TARGET_FORMAT, damage);
            let (r, stride) = (damage.unwrap_or(Region::whole(size)), padded_row_bytes(size.0));
            let offset = u64::from(r.y) * u64::from(stride) + u64::from(r.x) * 4;
            let layout =
                wgpu::TexelCopyBufferLayout { offset, bytes_per_row: Some(stride), rows_per_image: None };
            let copy = wgpu::TexelCopyBufferInfo { buffer: &target.readback, layout };
            let origin = wgpu::Origin3d { x: r.x, y: r.y, z: 0 };
            let from = wgpu::TexelCopyTextureInfo { origin, ..target.texture.as_image_copy() };
            let extent = wgpu::Extent3d { width: r.width, height: r.height, depth_or_array_layers: 1 };
            if !r.is_empty() {
                encoder.copy_texture_to_buffer(from, copy, extent);
            }
            let submission = self.queue.submit([encoder.finish()]);
            self.read(target, submission, |w, h, s, data| read(w, h, s, damage, data))
        });
        let read = rendered.flatten().inspect_err(|_| target.slots.clear())?;
        target.drawn = ages.serial();
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use crate::Renderer;
    use wprs_core::{Ages, Fit, Region};

    #[test]
    fn only_the_damaged_rectangle_is_drawn_and_read_back() {
        let r = match Renderer::shared() {
            Ok(r) => r,
            Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
        };
        let layer = |background| crate::Layer {
            image: None,
            fit: Fit::Fill,
            background,
            opacity: 1.0,
            uv: None,
            relief: None,
            flip: [false; 2],
            grade: None,
        };
        let (mut target, mut ages) = (r.create_target(40, 20).unwrap(), Ages::default());
        let mut seen = Vec::new();
        let mut frame = |color, damage, ages: &mut Ages, seen: &mut Vec<_>| {
            ages.push(damage);
            let got = r.render_damaged(&mut target, &[layer(color)], ages, |w, _, s, d, data| {
                let at = |x: u32, y: u32| data.get((y * s + x * 4) as usize..(y * s + x * 4 + 3) as usize);
                (d, at(0, 0).map(<[u8]>::to_vec), at(w - 1, 10).map(<[u8]>::to_vec))
            });
            seen.push(got.unwrap());
        };
        let red = Region { x: 30, y: 5, width: 10, height: 10 };
        frame([0, 0, 255], None, &mut ages, &mut seen);
        frame([255, 0, 0], Some(red), &mut ages, &mut seen);
        frame([0, 255, 0], Some(Region::default()), &mut ages, &mut seen);
        let (blue, reds) = (Some(vec![255, 0, 0]), Some(vec![0, 0, 255]));
        assert_eq!((seen[0].0, &seen[0].1, &seen[0].2), (None, &blue, &blue));
        assert_eq!((seen[1].0, &seen[1].1, &seen[1].2), (Some(red), &blue, &reds));
        assert_eq!(
            seen[2],
            (Some(Region::default()), blue.clone(), reds.clone()),
            "a clean frame draws nothing"
        );
        ages.push(Some(Region::whole((40, 20))));
        let mut other = Ages::default();
        other.push(Some(red));
        let whole = r.render_damaged(&mut target, &[layer([0, 0, 0])], &other, |_, _, _, d, _| d).unwrap();
        assert_eq!(whole, None, "frames of another history are drawn whole");
    }
}
