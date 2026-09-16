use std::net::SocketAddr;

pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://ciderhub.db".to_string());

        let bind_addr = std::env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
            .parse()
            .expect("BIND_ADDR must be a valid socket address, e.g. 127.0.0.1:3000");

        Self {
            database_url,
            bind_addr,
        }
    }
}
