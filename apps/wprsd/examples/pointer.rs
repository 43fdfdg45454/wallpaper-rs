use smithay_client_toolkit::reexports::client::globals::{GlobalListContents, registry_queue_init};
use smithay_client_toolkit::reexports::client::protocol::{wl_pointer::ButtonState, wl_registry::WlRegistry};
use smithay_client_toolkit::reexports::client::{Connection, Dispatch, QueueHandle, delegate_noop};
use smithay_client_toolkit::reexports::protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1 as Manager,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1 as Pointer,
};
use std::io::BufRead;

struct Seat;

impl Dispatch<WlRegistry, GlobalListContents> for Seat {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as smithay_client_toolkit::reexports::client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(Seat: ignore Manager);
delegate_noop!(Seat: ignore Pointer);

fn main() -> anyhow::Result<()> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<Seat>(&conn)?;
    let manager: Manager = globals.bind(&queue.handle(), 1..=2, ())?;
    let pointer = manager.create_virtual_pointer(None, &queue.handle(), ());
    queue.roundtrip(&mut Seat)?;
    println!("ready");
    for line in std::io::stdin().lock().lines() {
        let words: Vec<u32> = line?.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        match words[..] {
            [x, y, width, height] => pointer.motion_absolute(0, x, y, width, height),
            [button, down] => {
                let state = if down == 1 { ButtonState::Pressed } else { ButtonState::Released };
                pointer.button(0, 0x10f + button, state);
            }
            _ => continue,
        }
        pointer.frame();
        queue.roundtrip(&mut Seat)?;
        println!("done");
    }
    Ok(())
}
