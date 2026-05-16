use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::file::model::MetaData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFileTokenParams {
    #[serde(rename = "__type")]
    pub type_field: String,
    pub name: String,
    pub prefix: String,
    #[serde(rename = "metaData")]
    pub meta_data: MetaData,
    #[serde(rename = "ACL")]
    pub acl: HashMap<String, HashMap<String, bool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTokenResponse {
    #[serde(rename = "__type")]
    pub type_field: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
    pub key: String,
    pub name: String,
    pub token: String,
    #[serde(rename = "metaData")]
    pub meta_data: MetaData,
    #[serde(rename = "ACL")]
    pub acl: HashMap<String, HashMap<String, bool>>,
    pub bucket: String,
    #[serde(rename = "upload_url")]
    pub upload_url: String,
    pub url: String,
    pub provider: String,
    #[serde(rename = "mime_type")]
    pub mime_type: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadedPartInfo {
    #[serde(rename = "partNumber")]
    pub part_number: i32,
    pub etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteUploadParams {
    pub parts: Vec<UploadedPartInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartUploadResponse {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadPartResponse {
    pub etag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteUploadResponse {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCallbackResponse {
    pub result: bool,
}
