use std::collections::HashMap;

use chrono::DateTime;
use pcs_core::types::{
    ErrorCode, Metadata,
    backend::{PCSBackend, UserCheckResult},
    error::PCSError,
    event::Event,
    file_bucket::{FileBucket, ObjectMetadata, UploadedPart},
    kv::{KVStorage, KVTable},
};
use pcs_core::user::AuthData;
use serde::{Deserialize, Serialize};
use worker::*;

use crate::kv::WorkerKVStorage;
use crate::utils::{UnsafeSend, UnsafeStream};

#[derive(Serialize, Deserialize, Default)]
pub struct OpenIds(pub Vec<String>);

impl KVTable for OpenIds {
    const TABLE_NAME: &'static str = "_meta";
}

const ERROR_CODE: ErrorCode = ErrorCode::other(114);

pub struct WorkerBackend {
    pub db_kv: WorkerKVStorage,
    pub r2: Bucket,
    pub webhook: Option<String>,
    pub user_count_limit: u32,
}

impl PCSBackend for WorkerBackend {
    type FB = Self;
    type KV = WorkerKVStorage;

    fn fb(&self) -> &Self::FB {
        self
    }

    fn kv(&self) -> &Self::KV {
        &self.db_kv
    }

    async fn user_check(&self, auth: &AuthData) -> Result<UserCheckResult, PCSError> {
        if self.user_count_limit > 0 {
            let key = "user_count:openids";
            let mut openids: OpenIds = self
                .db_kv
                .get::<OpenIds>(key)
                .await
                .map_err(|e| PCSError::internal_error(ERROR_CODE, e.to_string()))?
                .unwrap_or_default();

            if !openids.0.contains(&auth.openid) {
                if openids.0.len() >= self.user_count_limit as usize {
                    return Err(PCSError::forbidden(ERROR_CODE, "user count limit reached"));
                }
                openids.0.push(auth.openid.clone());
                self.db_kv
                    .put(key, &openids)
                    .await
                    .map_err(|e| PCSError::internal_error(ERROR_CODE, e.to_string()))?;
            }
        }

        let Some(ref url) = self.webhook else {
            return Ok(UserCheckResult::default());
        };

        let body = serde_json::to_vec(auth)
            .map_err(|e| PCSError::internal_error(ERROR_CODE, e.to_string()))?;
        let webhook_url = format!("{}/pcs/user_check", url);

        let headers = Headers::new();
        headers
            .set("Content-Type", "application/json")
            .map_err(|e| PCSError::internal_error(ERROR_CODE, e.to_string()))?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(body.into()));

        let req = Request::new_with_init(&webhook_url, &init)
            .map_err(|e| PCSError::internal_error(ERROR_CODE, e.to_string()))?;

        let resp = UnsafeSend(async move { Fetch::Request(req).send().await })
            .await
            .map_err(|e| PCSError::internal_error(ERROR_CODE, e.to_string()))?;

        let status = resp.status_code();
        let data = match resp.body() {
            ResponseBody::Body(data) => data.clone(),
            _ => Vec::new(),
        };

        if status != 200 {
            return Err(PCSError::internal_error(
                ERROR_CODE,
                format!("webhook user_check returned status {}", status),
            ));
        }

        serde_json::from_slice(&data)
            .map_err(|e| PCSError::internal_error(ERROR_CODE, e.to_string()))
    }

    async fn emit_event(&self, event: Event) {
        let Some(ref url) = self.webhook else {
            return;
        };

        let body = match serde_json::to_vec(&event) {
            Ok(b) => b,
            Err(_) => return,
        };
        let webhook_url = format!("{}/pcs/event", url);

        let headers = Headers::new();
        let _ = headers.set("Content-Type", "application/json");

        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        init.with_headers(headers);
        init.with_body(Some(body.into()));

        let req = match Request::new_with_init(&webhook_url, &init) {
            Ok(r) => r,
            Err(_) => return,
        };

        let _ = UnsafeSend(async move { Fetch::Request(req).send().await }).await;
    }

    fn random_id(&self) -> String {
        random_id()
    }

    fn utc_now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }
}

fn random_id() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    const BASE: u128 = 36;

    let mut buf = [0u8; 18];
    getrandom::fill(&mut buf).expect("getrandom error");

    let mut n = 0u128;
    for &b in &buf {
        n = (n << 8) | b as u128;
    }

    let mut out = [0u8; 25];

    for i in (0..25).rev() {
        let idx = (n % BASE) as usize;
        out[i] = CHARSET[idx];
        n /= BASE;
    }

    String::from_utf8_lossy(&out).to_string()
}

pub struct R2MultipartUpload {
    upload: Option<worker::MultipartUpload>,
}

fn obj_meta_data(obj: &Object) -> Result<ObjectMetadata, worker::Error> {
    let update_at = DateTime::from_timestamp_millis(obj.uploaded().as_millis() as i64)
        .ok_or(worker::Error::RustError("time error".into()))?;
    Ok(ObjectMetadata {
        key: obj.key(),
        etag: obj.etag(),
        size: obj.size(),
        update_at,
        custom_metadata: obj.custom_metadata()?.into_iter().collect(),
    })
}

impl pcs_core::types::file_bucket::MultipartUpload for R2MultipartUpload {
    type Error = worker::Error;

    async fn upload_part(
        &mut self,
        part_number: u16,
        data: &[u8],
    ) -> Result<UploadedPart, Self::Error> {
        let upload = self
            .upload
            .as_ref()
            .ok_or(worker::Error::RustError("upload already completed".into()))?;
        let pn = part_number as u16;
        let part = UnsafeSend(async move { upload.upload_part(pn, data.to_vec()).await }).await?;
        Ok(UploadedPart::new(part.part_number(), part.etag()))
    }

    async fn complete(&mut self, parts: Vec<UploadedPart>) -> Result<ObjectMetadata, Self::Error> {
        let r2_parts: Vec<worker::UploadedPart> = parts
            .into_iter()
            .map(|p| worker::UploadedPart::new(p.part_number as u16, p.etag))
            .collect();

        let upload = self
            .upload
            .take()
            .ok_or(worker::Error::RustError("upload already completed".into()))?;

        let obj = UnsafeSend(async move { upload.complete(r2_parts).await }).await?;
        obj_meta_data(&obj)
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        if let Some(upload) = self.upload.take() {
            UnsafeSend(async move { upload.abort().await }).await?;
        }
        Ok(())
    }
}

impl FileBucket for WorkerBackend {
    type MultipartUpload = R2MultipartUpload;
    type Error = worker::Error;
    type Stream = UnsafeStream<worker::ByteStream>;

    async fn head(&self, key: &str) -> Result<Option<ObjectMetadata>, Self::Error> {
        let bucket = &self.r2;
        let obj = UnsafeSend(async move { bucket.head(key).await }).await?;

        obj.map(|obj| obj_meta_data(&obj)).transpose()
    }

    async fn get(&self, key: &str) -> Result<Option<(ObjectMetadata, Self::Stream)>, Self::Error> {
        let bucket = &self.r2;

        let obj = UnsafeSend(async move { bucket.get(key).execute().await }).await?;

        obj.map(|obj| {
            let metadata = obj_meta_data(&obj)?;

            let body = match obj.body() {
                Some(body) => body,
                None => return Ok(None),
            };

            let byte_stream = body.stream()?;

            Ok(Some((metadata, UnsafeStream(byte_stream))))
        })
        .transpose()
        .map(|x| x.flatten())
    }

    async fn delete(&self, key: &str) -> Result<(), Self::Error> {
        let bucket = &self.r2;
        UnsafeSend(async move { bucket.delete(key).await }).await?;
        Ok(())
    }

    async fn create_multipart_upload(
        &self,
        key: &str,
        meta_data: Metadata,
    ) -> Result<String, Self::Error> {
        let bucket = &self.r2;
        let map: HashMap<String, String> = meta_data.into_iter().collect();
        let upload = UnsafeSend(async move {
            bucket
                .create_multipart_upload(key)
                .custom_metadata(map)
                .execute()
                .await
        })
        .await?;
        Ok(UnsafeSend(async move { upload.upload_id().await }).await)
    }

    async fn get_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
    ) -> Result<Self::MultipartUpload, Self::Error> {
        let upload = self.r2.resume_multipart_upload(key, upload_id)?;
        Ok(R2MultipartUpload {
            upload: Some(upload),
        })
    }

    async fn put(
        &self,
        key: &str,
        data: &[u8],
        meta_data: Metadata,
    ) -> Result<ObjectMetadata, Self::Error> {
        let bucket = &self.r2;
        let map: HashMap<String, String> = meta_data.into_iter().collect();
        let obj = UnsafeSend(async move {
            bucket
                .put(key, data.to_vec())
                .custom_metadata(map)
                .execute()
                .await
        })
        .await?
        .ok_or(worker::Error::RustError("我不知道这里为什么会没有".into()))?;
        obj_meta_data(&obj)
    }
}
