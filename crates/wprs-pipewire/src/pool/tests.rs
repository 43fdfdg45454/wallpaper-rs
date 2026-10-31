use super::*;
use crate::params::META_TIMELINE;
use crate::syncobj::{SyncMeta, Timeline, UNSCHEDULED_RELEASE};

fn pool(count: usize) -> (Arc<Slot>, Vec<pw_buffer>) {
    let slot = Arc::new(Slot::default());
    let mut raw: Vec<pw_buffer> = (0..count).map(|_| unsafe { std::mem::zeroed() }).collect();
    let mut pool = lock(&slot.pool);
    for buffer in &mut raw {
        let index = pool.insert(Arc::new(()), buffer);
        pool.returned(index);
    }
    drop(pool);
    (slot, raw)
}

fn lease(slot: &Arc<Slot>) -> Option<Lease> {
    lock(&slot.pool).lease(slot)
}

fn present(mut lease: Lease) {
    lease.shown = true;
}

#[test]
fn rendering_never_touches_the_shown_buffer_and_the_newest_frame_wins() {
    let (slot, raw) = pool(3);
    assert!(lock(&slot.pool).starved());
    present(lease(&slot).expect("a free buffer"));
    assert!(!lock(&slot.pool).starved());
    assert_eq!(lock(&slot.pool).next(false), Some((raw.as_ptr().cast_mut(), None)));
    lock(&slot.pool).returned(0);
    let [a, b] = [lease(&slot), lease(&slot)].map(|l| l.expect("two free buffers"));
    assert_eq!((a.index, b.index), (1, 2));
    assert!(lease(&slot).is_none());
    drop(a);
    present(b);
    let again = lease(&slot).expect("the cancelled buffer");
    assert_eq!(again.index, 1);
    present(again);
    let pool = lock(&slot.pool);
    assert_eq!((pool.ready, pool.state(2)), (Some(1), Some(Use::Held)));
    drop(pool);
    assert_eq!(lock(&slot.pool).next(true).map(|n| n.0), Some(raw[1..].as_ptr().cast_mut()));
    assert_eq!(lock(&slot.pool).next(false), None);
}

#[test]
fn a_sent_buffer_carries_the_damage_of_every_frame_shown_since_the_last_one() {
    let (slot, _raw) = pool(3);
    let at = |x| Some(Region { x, y: 0, width: 1, height: 1 });
    let show = |damage| {
        let mut lease = lease(&slot).expect("a free buffer");
        lease.damage = damage;
        present(lease);
    };
    show(at(1));
    assert_eq!(lock(&slot.pool).next(false).map(|n| n.1), Some(None), "the first frame is whole");
    show(at(4));
    show(at(1));
    let both = Some(Region { x: 1, y: 0, width: 4, height: 1 });
    assert_eq!(lock(&slot.pool).next(false).map(|n| n.1), Some(both));
    (0..3).for_each(|i| lock(&slot.pool).returned(i));
    assert_eq!(lock(&slot.pool).next(true).map(|n| n.1), Some(Some(Region::default())));
}

#[test]
fn keep_alives_resend_the_newest_frame_and_old_leases_expire() {
    let (slot, raw) = pool(2);
    present(lease(&slot).expect("a free buffer"));
    let newest = lock(&slot.pool).next(false).map(|n| n.0);
    assert_eq!(lock(&slot.pool).next(true), None);
    lock(&slot.pool).returned(0);
    assert_eq!(lock(&slot.pool).next(true), newest.map(|n| (n, Some(Region::default()))));
    let stale = lease(&slot).expect("the other buffer");
    (0..2).for_each(|i| lock(&slot.pool).remove(i, raw[i..].as_ptr().cast_mut()));
    drop(stale);
    let pool = lock(&slot.pool);
    assert!(pool.buffers.is_empty() && !pool.active && pool.epoch == 1);
}

struct Fake(Arc<std::sync::atomic::AtomicU64>);

impl Timeline for Fake {
    fn signal(&self, point: u64) -> std::io::Result<()> {
        self.0.store(point, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn reached(&self) -> std::io::Result<u64> {
        Ok(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }
}

#[test]
fn a_buffer_is_drawn_into_again_only_after_its_release_point() {
    use pipewire::spa::sys::{spa_buffer, spa_meta};
    use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
    let (slot, mut raw) = pool(2);
    let blank = SyncMeta::default;
    let metas: Vec<*mut SyncMeta> = (0..2).map(|_| Box::into_raw(Box::new(blank()))).collect();
    let spa_metas: Vec<*mut spa_meta> = (metas.iter())
        .map(|&m| Box::into_raw(Box::new(spa_meta { type_: META_TIMELINE, size: 24, data: m.cast() })))
        .collect();
    let null = std::ptr::null_mut();
    let spa: Vec<*mut spa_buffer> = (spa_metas.iter())
        .map(|&m| Box::into_raw(Box::new(spa_buffer { n_metas: 1, n_datas: 0, metas: m, datas: null })))
        .collect();
    let (acquired, released) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
    for (i, buffer) in raw.iter_mut().enumerate() {
        buffer.buffer = spa[i];
        let fences = Fences::new(Box::new(Fake(acquired.clone())), Box::new(Fake(released.clone())));
        lock(&slot.pool).fence(i, fences);
    }
    present(lease(&slot).expect("a free buffer"));
    lock(&slot.pool).next(false);
    let sent = unsafe { &*metas[0] };
    assert_eq!((sent.acquire, sent.release, sent.flags), (1, 1, UNSCHEDULED_RELEASE));
    assert_eq!(acquired.load(SeqCst), 1, "the frame was not marked finished");
    present(lease(&slot).expect("the other buffer"));
    lock(&slot.pool).next(false);
    let first = metas[0];
    unsafe { (*first).flags = 0 };
    lock(&slot.pool).returned(0);
    assert!(lease(&slot).is_none(), "a buffer the consumer still reads was drawn into");
    released.store(1, SeqCst);
    assert_eq!(lease(&slot).map(|l| l.index), Some(0));
    drop(slot);
    for pointer in spa {
        drop(unsafe { Box::from_raw(pointer) });
    }
    spa_metas.into_iter().for_each(|m| drop(unsafe { Box::from_raw(m) }));
    metas.into_iter().for_each(|m| drop(unsafe { Box::from_raw(m) }));
}

#[test]
fn the_buffer_given_back_longest_ago_is_drawn_into_first() {
    let (slot, _raw) = pool(3);
    let frame = || {
        let lease = lease(&slot).expect("a free buffer");
        let index = lease.index;
        present(lease);
        lock(&slot.pool).next(false);
        (0..3).for_each(|i| lock(&slot.pool).returned(i));
        index
    };
    assert_eq!([(); 6].map(|()| frame()), [0, 1, 2, 0, 1, 2]);
}
