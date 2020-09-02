use anyhow::Result;

#[cfg(feature = "with_gtk")]
mod gtk;

#[cfg(feature = "with_x11")]
mod x11;

fn main() -> Result<()> {
    #[cfg(feature = "with_gtk")]
    if std::env::var("XDG_CURRENT_DESKTOP")
        .as_ref()
        .map(|x| &x[..])
        == Ok("GNOME")
    {
        gtk::gtk()?;
        return Ok(());
    }

    #[cfg(feature = "with_x11")]
    if std::env::var_os("DISPLAY").is_some() {
        x11::x11()?;
        return Ok(());
    }

    eprintln!("no supported clipboard found");
    Ok(())
}
