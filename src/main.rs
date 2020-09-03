use anyhow::Result;

#[cfg(feature = "with_x11")]
mod x11;

fn main() -> Result<()> {
    #[cfg(feature = "with_x11")]
    if std::env::var_os("DISPLAY").is_some() {
        x11::x11()?;
        return Ok(());
    }

    eprintln!("no supported clipboard found");
    Ok(())
}
