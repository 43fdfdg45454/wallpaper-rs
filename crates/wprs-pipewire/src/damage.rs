use crate::frame::{VideoFrame, copy_frame};
use pipewire::spa::sys as spa_sys;
use pipewire::sys::pw_buffer;
use spa_sys::{spa_meta_region, spa_point, spa_rectangle, spa_region};
use wprs_core::Region;

pub const META_DAMAGE: u32 = spa_sys::SPA_META_VideoDamage;
pub const REGIONS: usize = 16;

fn region(r: Region) -> spa_meta_region {
    let position = spa_point { x: r.x as i32, y: r.y as i32 };
    let size = spa_rectangle { width: r.width, height: r.height };
    spa_meta_region { region: spa_region { position, size } }
}

pub(crate) unsafe fn mark(buffer: *mut pw_buffer, damage: Option<Region>, size: (u32, u32)) -> bool {
    let Some(spa) = (unsafe { buffer.as_ref() }).and_then(|b| unsafe { b.buffer.as_ref() }) else {
        return false;
    };
    if spa.metas.is_null() {
        return false;
    }
    let metas = unsafe { std::slice::from_raw_parts(spa.metas, spa.n_metas as usize) };
    let Some(meta) = metas.iter().find(|m| m.type_ == META_DAMAGE && !m.data.is_null()) else { return false };
    let n = meta.size as usize / size_of::<spa_meta_region>();
    let regions = unsafe { std::slice::from_raw_parts_mut(meta.data.cast::<spa_meta_region>(), n) };
    regions.fill(region(Region::default()));
    let changed = damage.map_or(Region::whole(size), |r| r.within(size));
    if let Some(first) = regions.first_mut().filter(|_| !changed.is_empty()) {
        *first = region(changed);
    }
    true
}

pub(crate) unsafe fn write(
    buffer: *mut pw_buffer,
    frame: &VideoFrame,
    size: (u32, u32),
    since: Option<Region>,
) {
    let Some(data) = (unsafe { crate::memfd::first_data(buffer) }) else { return };
    let stride = size.0 * 4;
    let dst = (!data.data.is_null())
        .then(|| unsafe { std::slice::from_raw_parts_mut(data.data.cast::<u8>(), data.maxsize as usize) });
    let written = dst.map_or(0, |dst| copy_frame(frame, dst, size, since));
    if let Some(chunk) = unsafe { data.chunk.as_mut() } {
        (chunk.offset, chunk.stride, chunk.size) = (0, stride as i32, written as u32);
    }
}

#[derive(Default)]
pub(crate) struct Seen {
    buffers: Vec<(usize, u64)>,
    pub(crate) sent: u64,
}

impl Seen {
    pub(crate) fn swap(&mut self, buffer: *mut pw_buffer, serial: u64) -> u64 {
        let at = buffer.addr();
        match self.buffers.iter_mut().find(|b| b.0 == at) {
            Some(found) => std::mem::replace(&mut found.1, serial),
            None => {
                self.buffers.push((at, serial));
                0
            }
        }
    }

    pub(crate) fn forget(&mut self, buffer: *mut pw_buffer) {
        self.buffers.retain(|b| b.0 != buffer.addr());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffers_carry_the_damage_since_the_frame_sent_before() {
        let mut regions = [region(Region { x: 9, y: 9, width: 9, height: 9 }); 3];
        let mut meta = spa_sys::spa_meta {
            type_: META_DAMAGE,
            size: size_of_val(&regions) as u32,
            data: regions.as_mut_ptr().cast(),
        };
        let mut spa: spa_sys::spa_buffer = unsafe { std::mem::zeroed() };
        (spa.n_metas, spa.metas) = (1, &mut meta);
        let mut buffer: pw_buffer = unsafe { std::mem::zeroed() };
        buffer.buffer = &mut spa;
        let clock = Region { x: 1700, y: 20, width: 200, height: 60 };
        assert!(unsafe { mark(&mut buffer, Some(clock), (1920, 1080)) });
        let read = |r: &spa_meta_region| {
            let (p, s) = (r.region.position, r.region.size);
            (p.x, p.y, s.width, s.height)
        };
        assert_eq!(regions.map(|r| read(&r)), [(1700, 20, 200, 60), (0, 0, 0, 0), (0, 0, 0, 0)]);
        unsafe { mark(&mut buffer, None, (1920, 1080)) };
        assert_eq!(read(&regions[0]), (0, 0, 1920, 1080));
        unsafe { mark(&mut buffer, Some(Region::default()), (1920, 1080)) };
        assert_eq!(read(&regions[0]), (0, 0, 0, 0), "an unchanged frame says so");
        unsafe { (*buffer.buffer).n_metas = 0 };
        assert!(!unsafe { mark(&mut buffer, None, (1920, 1080)) });
    }

    #[test]
    fn each_buffer_remembers_the_frame_it_holds() {
        let mut seen = Seen::default();
        let [a, b] = [8usize, 16].map(std::ptr::without_provenance_mut::<pw_buffer>);
        assert_eq!((seen.swap(a, 3), seen.swap(b, 4)), (0, 0));
        assert_eq!(seen.swap(a, 5), 3);
        seen.forget(a);
        assert_eq!((seen.swap(a, 6), seen.swap(b, 7)), (0, 4), "a removed buffer starts over");
    }
}
