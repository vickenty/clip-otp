use std::{
    convert::TryInto,
    time::{Duration, Instant},
};

use anyhow::Result;
use notify_rust::{Notification, Timeout};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, SelectionNotifyEvent,
    SelectionRequestEvent, Window, WindowClass, SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::{res, Event};
use x11rb::{connection::Connection as _, wrapper::ConnectionExt as _};

use crate::{Conf, Pass};

type Conn = x11rb::rust_connection::RustConnection<x11rb::rust_connection::DefaultStream>;

#[path = "poll.rs"]
mod poll;

pub fn x11(conf: Conf, pass: Pass) -> Result<()> {
    let (cn, screen) = x11rb::connect(None)?;
    let screen = cn
        .setup()
        .roots
        .get(screen)
        .ok_or(anyhow::anyhow!("failed to retrieve screen {}", screen))?;

    let wid = cn.generate_id()?;
    cn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        wid,
        screen.root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_OUTPUT,
        screen.root_visual,
        &CreateWindowAux::new().event_mask(Some(EventMask::PROPERTY_CHANGE.into())),
    )?
    .check()?;

    let clipboard = intern(&cn, b"CLIPBOARD")?;
    let targets = intern(&cn, b"TARGETS")?;
    let utf8_string = intern(&cn, b"UTF8_STRING")?;
    let text_plain = intern(&cn, b"text/plain;charset=utf-8")?;

    cn.set_selection_owner(wid, clipboard, x11rb::CURRENT_TIME)?
        .check()?;

    let Conf {
        accept_list,
        mut reject_list,
        timeout,
    } = conf;

    let timeout = timeout.map(|tm| {
        Instant::now()
            .checked_add(Duration::from_millis(tm))
            .unwrap()
    });

    while let Some(ev) = wait_for_event(&cn, &timeout)? {
        if let Event::SelectionRequest(ev) = ev {
            let targ = cn.get_atom_name(ev.target)?.reply()?;
            let prop = cn.get_atom_name(ev.property)?.reply()?;
            let window_name = get_window_name(&cn, ev.requestor)?;
            let client_name = get_client_process_name(&cn, ev.requestor)?;

            debug!(
                "from {:x} ({:?} {:?}) target {:?} to {:?}",
                ev.requestor,
                window_name,
                client_name,
                String::from_utf8_lossy(&targ.name),
                String::from_utf8_lossy(&prop.name),
            );

            if prop.name.starts_with(b"META_SELECTION") {
                debug!("ignored");
                continue;
            }

            if ev.target == targets {
                respond(&cn, &ev, AtomEnum::ATOM, &[utf8_string])?;
            } else if ev.target == utf8_string || ev.target == text_plain {
                if reject_list.contains(&client_name) {
                    reject(&cn, &ev)?;
                    continue;
                }

                let action = if accept_list.contains(&client_name) {
                    Some("share".into())
                } else {
                    show_notification(&client_name, &window_name)?
                };

                match action.as_deref() {
                    Some("share") => {
                        respond(&cn, &ev, ev.target, pass.unlock())?;
                        break;
                    }
                    Some("clear") => break,
                    _ => {
                        reject(&cn, &ev)?;
                        reject_list.push(client_name.clone());
                    }
                }
            } else {
                debug!("unknown target");
                reject(&cn, &ev)?;
            }
        } else if let Event::SelectionClear(_) = ev {
            debug!("selection lost");
            break;
        } else {
            debug!("unknown req: {}", ev.response_type());
        }
    }

    Ok(())
}

fn intern(cn: &Conn, atom: &[u8]) -> Result<u32> {
    Ok(cn.intern_atom(true, atom)?.reply()?.atom)
}

trait Val {
    fn replace_property(&self, cn: &Conn, win: Window, prop: Atom, ty: Atom) -> Result<()>;
}
impl Val for &[u32] {
    fn replace_property(&self, cn: &Conn, win: Window, prop: Atom, ty: Atom) -> Result<()> {
        Ok(cn
            .change_property32(PropMode::REPLACE, win, prop, ty, self)?
            .check()?)
    }
}
impl Val for &[u8] {
    fn replace_property(&self, cn: &Conn, win: Window, prop: Atom, ty: Atom) -> Result<()> {
        Ok(cn
            .change_property8(PropMode::REPLACE, win, prop, ty, self)?
            .check()?)
    }
}
impl<T, const S: usize> Val for &[T; S]
where
    for<'a> &'a [T]: Val,
{
    fn replace_property(&self, cn: &Conn, win: Window, prop: Atom, ty: Atom) -> Result<()> {
        (&self[..]).replace_property(cn, win, prop, ty)
    }
}

fn respond(
    cn: &Conn,
    ev: &SelectionRequestEvent,
    ty: impl Into<Atom>,
    val: impl Val,
) -> Result<()> {
    val.replace_property(cn, ev.requestor, ev.property, ty.into())?;

    let n = SelectionNotifyEvent {
        response_type: SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: ev.time,
        requestor: ev.requestor,
        selection: ev.selection,
        target: ev.target,
        property: ev.property,
    };

    cn.send_event(false, ev.requestor, EventMask::NO_EVENT, n)?
        .check()?;

    Ok(())
}

fn reject(cn: &Conn, ev: &SelectionRequestEvent) -> Result<()> {
    let n = SelectionNotifyEvent {
        response_type: SELECTION_NOTIFY_EVENT,
        sequence: 0,
        time: ev.time,
        requestor: ev.requestor,
        selection: ev.selection,
        target: ev.target,
        property: ev.property,
    };

    cn.send_event(false, ev.requestor, EventMask::NO_EVENT, n)?
        .check()?;

    Ok(())
}

fn get_client_pid(cn: &Conn, id: u32) -> Result<u32> {
    let sp = res::ClientIdSpec {
        client: id,
        mask: res::ClientIdMask::LOCAL_CLIENT_PID.into(),
    };
    let id = res::query_client_ids(cn, &[sp])?.reply()?;

    let pid = id
        .ids
        .into_iter()
        .flat_map(|id| id.value.first().cloned())
        .next()
        .ok_or_else(|| anyhow::anyhow!("server did not return client window id"))?;

    Ok(pid)
}

fn get_client_process_name(cn: &Conn, id: u32) -> Result<String> {
    let pid = get_client_pid(&cn, id)?;

    let mut path = std::path::PathBuf::from("/proc");
    path.push(format!("{}", pid));
    path.push("exe");

    let exe = path.read_link()?;
    Ok(exe.to_string_lossy().into_owned())
}

fn get_window_name(cn: &Conn, window: Window) -> Result<String> {
    let v = cn
        .get_property(false, window, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 64)?
        .reply()?;
    Ok(String::from_utf8_lossy(&v.value).into_owned())
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
        .action("reject", "Reject")
        .show()?
        .wait_for_action(|a| action = Some(a.to_owned()));

    Ok(action)
}

fn wait_for_event(cn: &Conn, timeout: &Option<Instant>) -> Result<Option<Event>> {
    if let Some(timeout) = timeout {
        loop {
            match cn.poll_for_event()? {
                Some(ev) => return Ok(Some(ev)),
                None => {
                    let rem = match timeout.checked_duration_since(Instant::now()) {
                        Some(d) => d.as_millis().try_into().unwrap_or(i32::MAX),
                        None => return Ok(None),
                    };
                    poll::wait_with_timeout(cn.stream(), rem)?;
                }
            }
        }
    } else {
        Ok(Some(cn.wait_for_event()?))
    }
}
