use crate::file::FileTokenResponse;
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pointer {
    #[serde(rename = "__type")]
    pub type_field: String,
    #[serde(rename = "className")]
    pub class_name: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
}

impl Pointer {
    pub fn new(class_name: &str, object_id: &str) -> Self {
        Self {
            type_field: "Pointer".into(),
            class_name: class_name.into(),
            object_id: object_id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameDate {
    #[serde(rename = "__type")]
    pub type_field: String,
    pub iso: String,
}

impl GameDate {
    pub fn new(iso: impl Into<String>) -> Self {
        Self {
            type_field: "Date".into(),
            iso: iso.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSaveParams {
    pub summary: String,
    #[serde(rename = "gameFile")]
    pub game_file: Pointer,
    #[serde(rename = "modifiedAt")]
    pub modified_at: GameDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGameSaveParams {
    pub summary: String,
    pub name: String,
    #[serde(rename = "modifiedAt")]
    pub modified_at: GameDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGameSaveResponse {
    #[serde(rename = "objectId")]
    pub object_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSaveItem {
    pub summary: String,
    #[serde(rename = "gameFile")]
    pub game_file: FileTokenResponse,
    pub user: Pointer,
    #[serde(rename = "modifiedAt")]
    pub modified_at: GameDate,
    pub name: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListGameSaveResponse {
    pub results: Vec<GameSaveItem>,
}
