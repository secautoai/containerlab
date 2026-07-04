//! Optional Postgres + Redis persistence for NetPilot.
//!
//! Enable by setting `NETPILOT_DB_URL` (Postgres) — and optionally
//! `NETPILOT_REDIS_URL` for shared session storage; without Redis, bearer
//! tokens live in an in-process map (fine for a single instance). When
//! `NETPILOT_DB_URL` is unset the server runs in its original single-user,
//! file-only mode and none of this is touched.

mod model;
mod sessions;
mod store;

pub use model::{
    Access, AgentSessionMeta, FirmwareImage, LabMeta, Principal, Role, ShareGrant, User,
    Visibility,
};
pub use sessions::TokenStore;
pub use store::Db;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migrate: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("redis: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("password hashing failed")]
    Hash,
    #[error("{0}")]
    Conflict(String),
    #[error("not found")]
    NotFound,
    #[error("invalid credentials")]
    BadCredentials,
}

pub type Result<T> = std::result::Result<T, DbError>;
