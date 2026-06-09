use crate::{
    types::{KVTable, OCTET_STREAM_CONTENT_TYPE, backend::PCSBackend},
    utils::ToRfc3339Z,
};
use alloc::{
    format,
    string::{String, ToString},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::file::types::FileTokenResponse;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaData {
    pub size: u64,
    #[serde(rename = "_checksum")]
    pub checksum: String,
    pub prefix: String,
}

impl MetaData {
    pub fn new(size: u64, checksum: impl Into<String>, prefix: impl Into<String>) -> Self {
        Self {
            size,
            checksum: checksum.into(),
            prefix: prefix.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileToken {
    pub key: String,
    pub meta_data: MetaData,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl KVTable for FileToken {
    const TABLE_NAME: &'static str = "file_tokens";
}

impl FileToken {
    pub fn new(meta_data: MetaData, backend: &impl PCSBackend) -> Self {
        let now = backend.utc_now();
        Self {
            key: backend.random_id(),
            meta_data,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn to_response(&self, server_url: &str) -> FileTokenResponse {
        FileTokenResponse {
            type_field: "File".into(),
            object_id: self.key.clone(),
            key: self.key.clone(),
            name: ".save".into(),
            token: "unknown".into(),
            meta_data: self.meta_data.clone(),
            bucket: "file".into(),
            upload_url: server_url.to_string(),
            url: format!("{}/1.1/files/{}", server_url, self.key),
            provider: "qiniu".into(),
            mime_type: OCTET_STREAM_CONTENT_TYPE.into(),
            created_at: self.created_at.to_rfc3339_z(),
            updated_at: self.updated_at.to_rfc3339_z(),
        }
    }
}
