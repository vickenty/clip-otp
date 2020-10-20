use std::{fs::read_to_string, io::ErrorKind};
use serde::Deserialize;

use anyhow::Result;

#[cfg(feature = "with_x11")]
mod x11;

#[derive(Default, Debug, Deserialize)]
pub struct Conf {
    pub accept_list: Vec<String>,
    pub reject_list: Vec<String>,
}

fn main() -> Result<()> {
    let conf = load_conf()?;
    println!("{:?}", conf);

    #[cfg(feature = "with_x11")]
    if std::env::var_os("DISPLAY").is_some() {
        x11::x11(conf)?;
        return Ok(());
    }

    eprintln!("no supported clipboard found");
    Ok(())
}

fn load_conf() -> Result<Conf> {
    let conf = Conf::default();
    let dirs = xdg::BaseDirectories::new()?;
    let path = dirs.place_config_file("clip-otp.toml")?;

    println!("{}", path.to_string_lossy());

    let data = match read_to_string(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Ok(conf);
        }
        Err(e) => return Err(e.into()),
    };

    Ok(toml::from_str(&data)?)
}
