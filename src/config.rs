use std::fs;

use anyhow::Result;
use serde::Deserialize;

const PATH: &str = "config.toml";

#[derive(Deserialize)]
pub struct Bonfire {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct Config {
    pub bonfire: Bonfire,
}
impl Config {
    pub fn load() -> Result<Self> {
        let data = fs::read(PATH)?;
        toml::from_slice(&data).map_err(Into::into)
    }
}
