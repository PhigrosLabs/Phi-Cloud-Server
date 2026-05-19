use alloc::boxed::Box;
use alloc::string::String;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    types::{error::PCSError, event::Event, file_bucket::FileBucket, kv::KVStorage},
    user::AuthData,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCheckResult {
    pub name: Option<String>,
    pub short_id: Option<String>,
}

impl Default for UserCheckResult {
    fn default() -> Self {
        Self {
            name: None,
            short_id: Some("PCS".into()),
        }
    }
}

#[async_trait]
pub trait PCSBackend: Send + Sync + 'static {
    type FB: FileBucket;
    type KV: KVStorage;

    fn fb(&self) -> &Self::FB;
    fn kv(&self) -> &Self::KV;
    async fn user_check(&self, auth: &AuthData) -> Result<UserCheckResult, PCSError>;
    async fn emit_event(&self, event: Event);
    fn server_url(&self) -> String;
    fn random_id(&self) -> String;
    fn utc_now(&self) -> DateTime<Utc>;
}
