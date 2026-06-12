use crate::{file::LCFile, types::Date};
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

// {
//   "summary": "{summary}",
//   "modifiedAt": {
//     "__type": "Date",
//     "iso": "{rfc3339z}"
//   },
//   "gameFile": {
//     "__type": "Pointer",
//     "className": "_File",
//     "objectId": "{file_obj_id}"
//   },
//   "ACL": {
//     "{user_obj_id}": {
//       "read": true,
//       "write": true
//     }
//   },
//   "user": {
//     "__type": "Pointer",
//     "className": "_User",
//     "objectId": "{user_obj_id}"
//   }
// }
#[derive(Debug, Deserialize)]
pub struct GameSaveBody {
    pub summary: String,
    #[serde(rename = "gameFile")]
    pub game_file: Pointer,
    #[serde(rename = "modifiedAt")]
    pub modified_at: Date,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGameSaveBody {
    pub summary: String,
    pub name: String,
    #[serde(rename = "modifiedAt")]
    pub modified_at: Date,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutGameSaveResponse {
    #[serde(rename = "objectId")]
    pub object_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

// {
//     "createdAt": "{rfc3339z}",
//     "gameFile": {LCFile},
//     "modifiedAt": {
//         "__type": "Date",
//         "iso": "{rfc3339z}"
//     },
//     "name": "save",
//     "objectId": "{game_save_obj_id}",
//     "summary": "{summary}",
//     "updatedAt": "{rfc3339z}",
//     "user": {
//         "__type": "Pointer",
//         "className": "_User",
//         "objectId": "{user_obj_id}"
//     }
// }
#[derive(Debug, Serialize)]
pub struct GetGameSaveResponse {
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "gameFile")]
    pub game_file: LCFile,
    #[serde(rename = "modifiedAt")]
    pub modified_at: Date,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
    #[serde(rename = "summary")]
    pub summary: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "user")]
    pub user: Pointer,
}

#[derive(Debug, Serialize)]
pub struct ListGameSaveResponse {
    #[serde(rename = "results")]
    pub results: Vec<GetGameSaveResponse>,
}
