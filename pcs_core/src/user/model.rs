use alloc::string::String;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::backend::PCSBackend;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub object_id: String,
    pub nickname: String,
    pub openid: String,
    pub session_token: String,
    pub short_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Session {
    pub fn new(
        nickname: impl Into<String>,
        openid: impl Into<String>,
        short_id: impl Into<String>,
        backend: &impl PCSBackend,
    ) -> Self {
        let now = backend.get_utc_now();
        Self {
            object_id: backend.random_id(),
            nickname: nickname.into(),
            openid: openid.into(),
            session_token: backend.random_id(),
            short_id: short_id.into(),
            created_at: now,
            updated_at: now,
        }
    }
}
