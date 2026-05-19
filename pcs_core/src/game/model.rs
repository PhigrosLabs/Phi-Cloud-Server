use alloc::string::String;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::backend::PCSBackend;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSave {
    pub summary: String,
    pub game_file_object_id: String,
    pub object_id: String,
    pub modified_at: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl GameSave {
    pub fn new(
        summary: impl Into<String>,
        game_file_object_id: impl Into<String>,
        modified_at: impl Into<String>,
        backend: &impl PCSBackend,
    ) -> Self {
        let now = backend.utc_now();
        Self {
            summary: summary.into(),
            game_file_object_id: game_file_object_id.into(),
            object_id: backend.random_id(),
            modified_at: modified_at.into(),
            created_at: now,
            updated_at: now,
        }
    }
}
