use anyhow::Result;

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

    let clipboard = xcb::intern_atom(&cn, true, "CLIPBOARD").get_reply()?.atom();
    let targets = xcb::intern_atom(&cn, true, "TARGETS").get_reply()?.atom();
    let utf8_string = xcb::intern_atom(&cn, true, "UTF8_STRING")
        .get_reply()?
        .atom();
    // TODO: Where this target is specified?
    let text_plain = xcb::intern_atom(&cn, true, "text/plain;charset=utf-8")
        .get_reply()?
        .atom();

    xcb::set_selection_owner(&cn, wid, clipboard, xcb::CURRENT_TIME).request_check()?;

    while let Some(ev) = cn.wait_for_event() {
        if ev.response_type() == xcb::SELECTION_REQUEST {
            let ev: &xcb::SelectionRequestEvent = unsafe { xcb::cast_event(&ev) };

            let targ = xcb::get_atom_name(&cn, ev.target()).get_reply()?;
            let prop = xcb::get_atom_name(&cn, ev.property()).get_reply()?;
            let name = xcb::get_property(
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
                "from {} ({:?}) target {:?} to {:?}",
                ev.requestor(),
                String::from_utf8_lossy(name.value()),
                targ.name(),
                prop.name()
            );

            if prop.name().starts_with("META_SELECTION") {
                println!("ignored");
                continue;
            }

            if ev.target() == targets {
                respond(&cn, &ev, xcb::ATOM_ATOM, 32, &[utf8_string])?;
            } else if ev.target() == utf8_string || ev.target() == text_plain {
                respond(&cn, &ev, ev.target(), 8, b"Hello world")?;
                break;
            } else {
                println!("unknown target");
            }
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
