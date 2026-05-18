use async_trait::async_trait;
use futures::Stream;
use pcs_core::types::{
    backend::{PCSBackend, UserCheckResult},
    error::PCSError,
    event::Event,
    file_bucket::{FileBucket, ObjectMetadata, UploadedPart},
};
use pcs_core::user::AuthData;
use worker::*;

use crate::kv::WorkerKVStorage;
use crate::utils::{UnsafeSend, UnsafeStream};

pub struct WorkerBackend {
    pub db_kv: WorkerKVStorage,
    pub r2: Bucket,
    pub webhook: Option<String>,
    pub scheme: String,
}

#[async_trait]
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
        let Some(ref url) = self.webhook else {
            return Ok(UserCheckResult::default());
        };

        let body = serde_json::to_vec(auth).map_err(|e| PCSError::internal_error(e.to_string()))?;
        let webhook_url = format!("{}/pcs/user_check", url);

        let headers = Headers::new();
        headers
            .set("Content-Type", "application/json")
            .map_err(|e| PCSError::internal_error(e.to_string()))?;

        let mut init = RequestInit::new();
        init.with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(body.into()));

        let req = Request::new_with_init(&webhook_url, &init)
            .map_err(|e| PCSError::internal_error(e.to_string()))?;

        let resp = UnsafeSend(async move { Fetch::Request(req).send().await })
            .await
            .map_err(|e| PCSError::internal_error(e.to_string()))?;

        let status = resp.status_code();
        let data = match resp.body() {
            ResponseBody::Body(data) => data.clone(),
            _ => Vec::new(),
        };

        if status != 200 {
            return Err(PCSError::internal_error(format!(
                "webhook user_check returned status {}",
                status
            )));
        }

        serde_json::from_slice(&data).map_err(|e| PCSError::internal_error(e.to_string()))
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

    fn scheme(&self) -> String {
        self.scheme.clone()
    }

    fn random_id(&self) -> String {
        random_id()
    }

    fn get_utc_now(&self) -> chrono::DateTime<chrono::Utc> {
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

#[async_trait]
impl pcs_core::types::file_bucket::MultipartUpload for R2MultipartUpload {
    type Error = worker::Error;

    async fn upload_part(
        &mut self,
        part_number: u32,
        data: Vec<u8>,
    ) -> Result<UploadedPart, Self::Error> {
        let upload = self
            .upload
            .as_ref()
            .ok_or_else(|| worker::Error::RustError("upload already completed".into()))?;
        let pn = part_number as u16;
        let part = UnsafeSend(async move { upload.upload_part(pn, data).await }).await?;
        Ok(UploadedPart::new(part.part_number() as i32, part.etag()))
    }

    async fn complete(&mut self, parts: Vec<UploadedPart>) -> Result<ObjectMetadata, Self::Error> {
        let r2_parts: Vec<worker::UploadedPart> = parts
            .into_iter()
            .map(|p| worker::UploadedPart::new(p.part_number as u16, p.etag))
            .collect();

        let upload = self
            .upload
            .take()
            .ok_or_else(|| worker::Error::RustError("upload already completed".into()))?;

        let obj = UnsafeSend(async move { upload.complete(r2_parts).await }).await?;

        Ok(ObjectMetadata::new(obj.key(), obj.http_etag(), obj.size()))
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        if let Some(upload) = self.upload.take() {
            UnsafeSend(async move { upload.abort().await }).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl FileBucket for WorkerBackend {
    type MultipartUpload = R2MultipartUpload;
    type Error = PCSError;

    async fn head(&self, key: impl Into<String> + Send) -> Result<ObjectMetadata, Self::Error> {
        let bucket = &self.r2;
        let obj = UnsafeSend(async move { bucket.head(key).await })
            .await
            .map_err(|e| PCSError::internal_error(e.to_string()))?
            .ok_or_else(|| PCSError::not_found("object not found"))?;
        Ok(ObjectMetadata::new(obj.key(), obj.http_etag(), obj.size()))
    }

    async fn get(
        &self,
        key: impl Into<String> + Send,
    ) -> Result<impl Stream<Item = bytes::Bytes> + Send + Unpin + 'static, Self::Error> {
        use bytes::Bytes;
        use futures::{StreamExt, stream};

        let bucket = &self.r2;
        let obj = UnsafeSend(async move { bucket.get(key).execute().await })
            .await
            .map_err(|e| PCSError::internal_error(e.to_string()))?
            .ok_or_else(|| PCSError::not_found("object not found"))?;

        type PStream = UnsafeStream<std::pin::Pin<Box<dyn Stream<Item = Bytes> + 'static>>>;

        let raw: PStream = if let Some(body) = obj.body() {
            let bstream = body
                .stream()
                .map_err(|e| PCSError::internal_error(e.to_string()))?;
            UnsafeStream(Box::pin(bstream.filter_map(|res| async move {
                match res {
                    Ok(v) => Some(Bytes::from(v)),
                    Err(_) => None,
                }
            })))
        } else {
            UnsafeStream(Box::pin(stream::empty()))
        };
        Ok(raw)
    }

    async fn delete(&self, key: impl Into<String> + Send) -> Result<(), Self::Error> {
        let bucket = &self.r2;
        UnsafeSend(async move { bucket.delete(key).await })
            .await
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        Ok(())
    }

    async fn create_multipart_upload(
        &self,
        key: impl Into<String> + Send,
    ) -> Result<String, Self::Error> {
        let bucket = &self.r2;
        let upload = UnsafeSend(async move { bucket.create_multipart_upload(key).execute().await })
            .await
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        Ok(UnsafeSend(async move { upload.upload_id().await }).await)
    }

    async fn get_multipart_upload(
        &self,
        key: impl Into<String> + Send,
        upload_id: impl Into<String> + Send,
    ) -> Result<Self::MultipartUpload, Self::Error> {
        let upload = self
            .r2
            .resume_multipart_upload(key, upload_id)
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        Ok(R2MultipartUpload {
            upload: Some(upload),
        })
    }
}
