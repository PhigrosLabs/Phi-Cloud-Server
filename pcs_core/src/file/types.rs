use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    types::{
        ACL, ErrorCode, FileBucket, OCTET_STREAM_CONTENT_TYPE, ObjectMetadata, PCSBackend, PCSError,
    },
    utils::{MapPCSError, ToRfc3339Z},
};

pub const FILE_UPTOKEN: &str = "unknown";

// {
//   "name": ".save",
//   "__type": "File",
//   "ACL": {
//     "{user_obj_id}": {
//       "read": true,
//       "write": true
//     }
//   },
//   "prefix": "gamesaves",
//   "metaData": {
//     "size": {size},
//     "_checksum": "{md5}",
//     "prefix": "gamesaves"
//   }
// }
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFileTokenBody {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "__type")]
    pub r#type: String,
    #[serde(rename = "ACL")]
    pub acl: ACL,
    #[serde(rename = "prefix")]
    pub prefix: String,
    #[serde(rename = "metaData")]
    pub meta_data: FileTokenMetaData,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct FileTokenMetaData {
    #[serde(rename = "_checksum")]
    pub checksum: String,
    #[serde(rename = "prefix")]
    pub prefix: String,
    #[serde(rename = "size")]
    pub size: u64,
}

impl From<FileTokenMetaData> for Vec<(String, String)> {
    fn from(meta: FileTokenMetaData) -> Self {
        vec![
            ("_checksum".into(), meta.checksum),
            ("prefix".into(), meta.prefix),
            ("size".into(), meta.size.to_string()),
        ]
    }
}

impl From<Vec<(String, String)>> for FileTokenMetaData {
    fn from(v: Vec<(String, String)>) -> Self {
        let mut meta = FileTokenMetaData::default();

        for (k, val) in v {
            match k.as_str() {
                "_checksum" => meta.checksum = val,
                "prefix" => meta.prefix = val,
                "size" => meta.size = val.parse().unwrap_or_default(),
                _ => {}
            }
        }

        meta
    }
}

// {
//   "bucket": "rAK3Ffdi",
//   "createdAt": "{rfc3339z}",
//   "key": "{key}",
//   "metaData": {
//     "_checksum": "{md5}",
//     "prefix": "gamesaves",
//     "size": {size}
//   },
//   "mime_type": "application/octet-stream",
//   "name": ".save",
//   "objectId": "{file_obj_id}",
//   "provider": "qiniu",
//   "token": "{UpToken}",
//   "upload_url": "{server_url}",
//   "url": "{file_url}"
// }
#[derive(Debug, Serialize)]
pub struct CreateFileTokenResponse {
    #[serde(rename = "bucket")]
    pub bucket: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "key")]
    pub key: String,
    #[serde(rename = "metaData")]
    pub meta_data: FileTokenMetaData,
    #[serde(rename = "mime_type")]
    pub mime_type: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
    #[serde(rename = "provider")]
    pub provider: String,
    #[serde(rename = "token")]
    pub token: String,
    #[serde(rename = "upload_url")]
    pub upload_url: String,
    #[serde(rename = "url")]
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadedPartInfo {
    #[serde(rename = "partNumber")]
    pub part_number: u16,
    pub etag: String,
}

#[derive(Debug, Deserialize)]
pub struct CompleteUploadBody {
    pub parts: Vec<UploadedPartInfo>,
}

#[derive(Debug, Serialize)]
pub struct StartUploadResponse {
    #[serde(rename = "uploadId")]
    pub upload_id: String,
}

// {
//   "etag": "{etag}",
//   "md5": "{?}"
// }
#[derive(Debug, Serialize)]
pub struct UploadPartResponse {
    pub etag: String,
}

// {
//   "hash": "{?}",
//   "key": "{key}"
// }
#[derive(Debug, Serialize)]
pub struct CompleteUploadResponse {
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct FileCallbackResponse {
    pub result: bool,
    pub token: String,
}

#[derive(Debug)]
pub struct FileTokenInfo {
    pub key: String,
    pub meta_data: Vec<(String, String)>,
    pub updated_at: DateTime<Utc>,
    pub upload_id: Option<String>,
}

// {
//     "__type": "File",
//     "bucket": "rAK3Ffdi",
//     "createdAt": "{rfc3339z}",
//     "key": "{key}",
//     "metaData": {
//         "_checksum": "{md5}",
//         "prefix": "{prefix}",
//         "size": {size}
//     },
//     "mime_type": "application/octet-stream",
//     "name": ".save",
//     "objectId": "{file_obj_id}",
//     "provider": "qiniu",
//     "updatedAt": "{rfc3339z}",
//     "url": "{file_url}"
// }
#[derive(Debug, Serialize)]
pub struct LCFile {
    #[serde(rename = "__type")]
    pub r#type: String,
    #[serde(rename = "bucket")]
    pub bucket: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "key")]
    pub key: String,
    #[serde(rename = "metaData")]
    pub meta_data: FileTokenMetaData,
    #[serde(rename = "mime_type")]
    pub mime_type: String,
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
    #[serde(rename = "provider")]
    pub provider: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "url")]
    pub url: String,
}

impl FileTokenInfo {
    pub async fn new(
        backend: &impl PCSBackend,
        meta_data: Vec<(String, String)>,
    ) -> Result<Self, PCSError> {
        let key = backend.random_id();
        let fb = backend.fb();
        let upload_id = fb
            .create_multipart_upload(&key, meta_data.clone())
            .await
            .map_pcs_error(ErrorCode::FB_CREATE_MULTIPART_UPLOAD)?;
        Ok(Self {
            key,
            upload_id: Some(upload_id),
            meta_data: meta_data.clone(),
            updated_at: backend.utc_now(),
        })
    }

    pub fn get_from_meta_data(meta_data: ObjectMetadata) -> Self {
        Self {
            key: meta_data.key,
            meta_data: meta_data.custom_metadata,
            updated_at: meta_data.update_at,
            upload_id: None,
        }
    }

    pub fn to_lc_file(&self, server_url: &str) -> LCFile {
        LCFile {
            r#type: "File".into(),
            bucket: self.upload_id.clone().unwrap_or_else(|| "file".to_string()),
            created_at: self.updated_at.to_rfc3339_z(),
            updated_at: self.updated_at.to_rfc3339_z(),
            key: self.key.clone(),
            meta_data: self.meta_data.clone().into(),
            mime_type: OCTET_STREAM_CONTENT_TYPE.into(),
            name: ".save".into(),
            object_id: self.key.clone(),
            provider: "qiniu".into(),
            url: format!("{}/1.1/files/{}", server_url, self.key),
        }
    }

    pub fn to_response(&self, server_url: &str) -> CreateFileTokenResponse {
        CreateFileTokenResponse {
            object_id: self.key.clone(),
            key: self.key.clone(),
            name: ".save".into(),
            token: FILE_UPTOKEN.into(),
            meta_data: self.meta_data.clone().into(),
            bucket: self.upload_id.clone().unwrap_or("file".to_string()),
            upload_url: server_url.to_string(),
            url: format!("{}/1.1/files/{}", server_url, self.key),
            provider: "qiniu".into(),
            mime_type: OCTET_STREAM_CONTENT_TYPE.into(),
            created_at: self.updated_at.to_rfc3339_z(),
        }
    }
}
