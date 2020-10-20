use anyhow::Result;

use notify_rust::{Notification, Timeout};

use xcb::Event;

pub fn x11() -> Result<()> {
    let (cn, screen) = xcb::Connection::connect(None)?;
    let screen = cn
        .get_setup()
        .roots()
        .nth(screen as usize)
        .ok_or(anyhow::anyhow!("failed to retrieve screen {}", screen))?;

    let wid = cn.generate_id();
    xcb::create_window(
        &cn,
        screen.root_depth(),
        wid,
        screen.root(),
        0,
        0,
        1,
        1,
        0,
        xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
        screen.root_visual(),
        &[(xcb::CW_EVENT_MASK, xcb::EVENT_MASK_PROPERTY_CHANGE)],
    )
    .request_check()?;

    let clipboard = xcb::intern_atom(&cn, true, b"CLIPBOARD")
        .get_reply()?
        .atom();
    let targets = xcb::intern_atom(&cn, true, b"TARGETS").get_reply()?.atom();
    let utf8_string = xcb::intern_atom(&cn, true, b"UTF8_STRING")
        .get_reply()?
        .atom();
    // TODO: Where this target is specified?
    let text_plain = xcb::intern_atom(&cn, true, b"text/plain;charset=utf-8")
        .get_reply()?
        .atom();

    xcb::set_selection_owner(&cn, wid, clipboard, xcb::CURRENT_TIME).request_check()?;

    while let Some(ev) = cn.wait_for_event() {
        if let Some(ev) = xcb::SelectionRequestEvent::try_cast(&cn, &ev) {
            let targ = xcb::get_atom_name(&cn, ev.target()).get_reply()?;
            let prop = xcb::get_atom_name(&cn, ev.property()).get_reply()?;
            let window_name = xcb::get_property(
                &cn,
                false,
                ev.requestor(),
                xcb::ATOM_WM_NAME,
                xcb::ATOM_ANY,
                0,
                64,
            )
            .get_reply()?;

            println!(
                "from {:x} ({:?} {:?}) target {:?} to {:?}",
                ev.requestor(),
                String::from_utf8_lossy(window_name.value()),
                get_client_process_name(&cn, ev.requestor())?,
                targ.name(),
                prop.name()
            );

            if prop.name().starts_with(b"META_SELECTION") {
                println!("ignored");
                continue;
            }

            if ev.target() == targets {
                respond(&cn, &ev, xcb::ATOM_ATOM, 32, &[utf8_string])?;
            } else if ev.target() == utf8_string || ev.target() == text_plain {
                let action = show_notification(
                    &get_client_process_name(&cn, ev.requestor())?,
                    &String::from_utf8_lossy(window_name.value()),
                )?;

                match action.as_deref() {
                    Some("share") => {
                        respond(&cn, &ev, ev.target(), 8, b"Hello world")?;
                        break;
                    }
                    Some("clear") => break,
                    Some("__closed") => {
                        reject(&cn, &ev)?;
                    }
                    _ => continue,
                }
            } else {
                println!("unknown target");
                reject(&cn, &ev)?;
            }
        } else if let Some(_ev) = xcb::SelectionClearEvent::try_cast(&cn, &ev) {
            println!("selection lost");
            break;
        } else {
            println!("unknown req: {}", ev.response_type());
        }
        cn.has_error()?;
    }

    Ok(())
}

fn respond<T>(
    cn: &xcb::Connection,
    ev: &xcb::SelectionRequestEvent,
    ty: xcb::Atom,
    w: u8,
    v: &[T],
) -> Result<()> {
    xcb::change_property(
        cn,
        xcb::PROP_MODE_REPLACE as u8,
        ev.requestor(),
        ev.property(),
        ty,
        w,
        v,
    )
    .request_check()?;

    let n = xcb::SelectionNotifyEvent::new(
        ev.time(),
        ev.requestor(),
        ev.selection(),
        ev.target(),
        ev.property(),
    );

    xcb::send_event(&cn, false, ev.requestor(), 0, &n).request_check()?;

    Ok(())
}

fn reject(cn: &xcb::Connection, ev: &xcb::SelectionRequestEvent) -> Result<()> {
    let n = xcb::SelectionNotifyEvent::new(
        ev.time(),
        ev.requestor(),
        ev.selection(),
        ev.target(),
        xcb::NONE,
    );

    xcb::send_event(&cn, false, ev.requestor(), 0, &n).request_check()?;

    Ok(())
}

fn get_client_pid(cn: &xcb::Connection, id: u32) -> Result<u32> {
    let sp = xcb::res::ClientIdSpec::new(id, xcb::res::CLIENT_ID_MASK_LOCAL_CLIENT_PID);
    let id = xcb::res::query_client_ids(&cn, &[sp]).get_reply()?;

    let pid = id
        .ids()
        .flat_map(|id| id.value().first().cloned())
        .next()
        .ok_or_else(|| anyhow::anyhow!("server did not return client window id"))?;

    Ok(pid)
}

fn get_client_process_name(cn: &xcb::Connection, id: u32) -> Result<String> {
    let pid = get_client_pid(&cn, id)?;

    let mut path = std::path::PathBuf::from("/proc");
    path.push(format!("{}", pid));
    path.push("exe");

    let exe = path.read_link()?;
    Ok(exe.to_string_lossy().into_owned())
}

fn show_notification(client_name: &str, window_name: &str) -> Result<Option<String>> {
    let mut action = None;

    Notification::new()
        .summary("Clip Otp")
        .body(&format!(
            "Share password in clipboard with\n{:?} ({:?})",
            client_name, window_name,
        ))
        .icon("dialog-password")
        .sound_name("window-attention-active")
        .urgency(notify_rust::Urgency::Critical)
        .timeout(Timeout::Never)
        .action("share", "Share")
        .action("clear", "Clear")
        .show()?
        .wait_for_action(|a| action = Some(a.to_owned()));

    Ok(action)
}
