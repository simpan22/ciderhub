use std::net::SocketAddr;

pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub app_password: String,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://ciderhub.db".to_string());

        let bind_addr = std::env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
            .parse()
            .expect("BIND_ADDR must be a valid socket address, e.g. 127.0.0.1:3000");

        // No fallback default — this used to be a hardcoded constant in
        // source, which is fine for a private repo but not one meant to
        // go public. Failing fast at startup if it's missing is safer
        // than silently falling back to some other hardcoded value.
        let app_password = std::env::var("APP_PASSWORD")
            .expect("APP_PASSWORD must be set (see .env.example) — the shared site password");

        Self {
            database_url,
            bind_addr,
            app_password,
        }
    }
}
