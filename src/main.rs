use serde::Deserialize;
use std::{
    fs::read_to_string,
    io::{ErrorKind, Read},
};

use anyhow::Result;

#[macro_use]
mod log;
mod x11;

#[derive(Default, Debug, Deserialize)]
pub struct Conf {
    pub accept_list: Vec<String>,
    pub reject_list: Vec<String>,
    pub timeout: Option<u64>,
}

pub struct Pass(Vec<u8>);

impl Pass {
    fn unlock(&self) -> &[u8] {
        &self.0
    }
}

fn main() -> Result<()> {
    let conf = load_conf()?;
    debug!("{:?}", conf);

    let mut pass = Pass(Vec::new());
    std::io::stdin().read_to_end(&mut pass.0)?;

    print!("Copied password to clipboard");
    if let Some(timeout) = conf.timeout {
        print!(", will clear after {} seconds", timeout / 1000);
    }
    println!(".");

    if std::env::var_os("DISPLAY").is_some() {
        x11::x11(conf, pass)?;
        return Ok(());
    }

    eprintln!("no supported clipboard found");
    Ok(())
}

fn load_conf() -> Result<Conf> {
    let conf = Conf::default();
    let dirs = xdg::BaseDirectories::new()?;
    let path = dirs.place_config_file("clip-otp.toml")?;

    debug!("{}", path.to_string_lossy());

    let data = match read_to_string(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Ok(conf);
        }
        Err(e) => return Err(e.into()),
    };

    Ok(toml::from_str(&data)?)
}
