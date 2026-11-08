use std::fs;

use anyhow::Result;
use bonfire::fcm::models::{Credentials as FcmCredentials, Subscription};
use bonfire::models::Auth;
use serde::{Deserialize, Serialize};

const PATH: &str = "credentials.toml";

#[derive(Deserialize, Serialize)]
pub struct Credentials {
    pub auth: Auth,
    pub fcm: FcmCredentials,
    pub subscription: Option<Subscription>,
}
impl Credentials {
    pub fn load() -> Result<Option<Self>> {
        fs::read(PATH)
            .ok()
            .map(|data| toml::from_slice(&data))
            .transpose()
            .map_err(Into::into)
    }

    pub fn save(&self) -> Result<()> {
        let data = toml::to_string(self)?;
        fs::write(PATH, data)?;
        Ok(())
    }
}
