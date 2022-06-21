use std::{env, path::PathBuf};

use sai::{Component, ComponentLifecycle};

#[derive(Component)]
#[lifecycle]
pub struct Config {
    port: u16,
    base_path: PathBuf,
    auth_url: String,
    old_auth_url: String,
}

#[async_trait::async_trait]
impl ComponentLifecycle for Config {
    async fn start(&mut self) {
        dotenv::dotenv().ok();

        let port = env::var("PORT")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(30001);

        let base_path = env::var("BASE_PATH").unwrap();

        self.port = port;
        self.base_path = PathBuf::new().join(base_path);
    }
}

impl Config {
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn base_path(&self) -> PathBuf {
        self.base_path.clone()
    }

    pub fn auth_url(&self) -> &str {
        self.auth_url.as_str()
    }

    pub fn old_auth_url(&self) -> &str {
        self.old_auth_url.as_str()
    }
}
