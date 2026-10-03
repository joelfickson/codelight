mod chatgpt;
pub mod chatgpt_auth;
mod models;

pub use models::gateway::{DEFAULT_MODEL, GatewayClient, GatewayError};
