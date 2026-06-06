use alloc::string::String;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::backend::PCSBackend;
use crate::types::kv::KVTable;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokenByOpenId(pub String);

impl KVTable for SessionTokenByOpenId {
    const TABLE_NAME: &'static str = "sessions_by_openid";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokenByObjId(pub String);

impl KVTable for SessionTokenByObjId {
    const TABLE_NAME: &'static str = "sessions_by_objid";
}

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

impl KVTable for Session {
    const TABLE_NAME: &'static str = "sessions";
}

impl Session {
    pub fn new(
        nickname: impl Into<String>,
        openid: impl Into<String>,
        short_id: impl Into<String>,
        backend: &impl PCSBackend,
    ) -> Self {
        let now = backend.utc_now();
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
