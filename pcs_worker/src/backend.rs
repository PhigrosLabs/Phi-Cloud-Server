use async_trait::async_trait;
use pcs_core::types::{
    backend::{PCSBackend, UserCheckResult},
    error::PCSError,
    event::Event,
    file_bucket::{FileBucket, ObjectMetadata, UploadedPart},
};
use pcs_core::user::AuthData;
use worker::*;

use crate::kv::WorkerKVStorage;
use crate::utils::UnsafeSend;

pub struct WorkerBackend {
    pub db_kv: WorkerKVStorage,
    pub file_kv: Option<KvStore>,
    pub r2: Option<Bucket>,
    pub file_mode: String,
    pub webhook: Option<String>,
    pub scheme: String,
}

#[async_trait]
impl PCSBackend for WorkerBackend {
    type FB = Self;
    type KV = WorkerKVStorage;

    async fn file_bucket(&self) -> &Self::FB {
        self
    }

    async fn kv(&self) -> &Self::KV {
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

fn kv_object_etag(data: &[u8]) -> String {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    format!("{:x}-{:x}", h, data.len())
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

        Ok(ObjectMetadata::new(
            obj.key(),
            obj.http_etag(),
            obj.size(),
            obj.http_metadata().content_type,
        ))
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        if let Some(upload) = self.upload.take() {
            UnsafeSend(async move { upload.abort().await }).await?;
        }
        Ok(())
    }
}

pub struct KvMultipartUpload {
    kv: KvStore,
    key: String,
    upload_id: String,
}

#[async_trait]
impl pcs_core::types::file_bucket::MultipartUpload for KvMultipartUpload {
    type Error = worker::Error;

    async fn upload_part(
        &mut self,
        part_number: u32,
        data: Vec<u8>,
    ) -> Result<UploadedPart, Self::Error> {
        let part_key = format!(
            "{}:upload:{}:part:{}",
            self.key, self.upload_id, part_number
        );
        let etag = kv_object_etag(&data);
        let kv = self.kv.clone();
        UnsafeSend(async move { kv.put_bytes(&part_key, &data)?.execute().await }).await?;

        // Track part number in state
        let state_key = format!("{}:upload:{}:state", self.key, self.upload_id);
        let state_key2 = state_key.clone();
        let kv = self.kv.clone();
        let mut parts: Vec<u32> = UnsafeSend(async move {
            let existing = kv.get(&state_key).json::<Vec<u32>>().await?;
            Ok::<_, worker::Error>(existing.unwrap_or_default())
        })
        .await?;
        if !parts.contains(&part_number) {
            parts.push(part_number);
        }
        let kv = self.kv.clone();
        UnsafeSend(async move {
            kv.put(&state_key2, serde_json::to_vec(&parts)?)?
                .execute()
                .await
        })
        .await?;

        Ok(UploadedPart::new(part_number as i32, etag))
    }

    async fn complete(&mut self, _parts: Vec<UploadedPart>) -> Result<ObjectMetadata, Self::Error> {
        let state_key = format!("{}:upload:{}:state", self.key, self.upload_id);
        let kv = self.kv.clone();
        let part_numbers: Vec<u32> = UnsafeSend(async move {
            let existing = kv.get(&state_key).json::<Vec<u32>>().await?;
            Ok::<_, worker::Error>(existing.unwrap_or_default())
        })
        .await?;

        let mut all_data = Vec::new();
        for pn in &part_numbers {
            let part_key = format!("{}:upload:{}:part:{}", self.key, self.upload_id, pn);
            let kv = self.kv.clone();
            let data: Option<Vec<u8>> = UnsafeSend(async move {
                Ok::<_, worker::Error>(
                    kv.get(&part_key)
                        .bytes()
                        .await
                        .map_err(worker::Error::from)?,
                )
            })
            .await?;
            if let Some(d) = data {
                all_data.extend_from_slice(&d);
            }
        }

        let size = all_data.len() as u64;
        let etag = kv_object_etag(&all_data);

        // Store final object
        let kv = self.kv.clone();
        let key = self.key.clone();
        UnsafeSend(async move { kv.put_bytes(&key, &all_data)?.execute().await }).await?;

        // Store metadata
        let meta_key = format!("{}:meta", self.key);
        let meta = serde_json::json!({
            "size": size,
            "etag": etag.clone(),
            "content_type": "application/octet-stream",
        });
        let kv = self.kv.clone();
        UnsafeSend(async move {
            kv.put(&meta_key, serde_json::to_vec(&meta)?)?
                .execute()
                .await
        })
        .await?;

        // Cleanup upload temp keys
        let kv = self.kv.clone();
        let prefix = format!("{}:upload:{}:", self.key, self.upload_id);
        UnsafeSend(async move {
            let list = kv.list().prefix(prefix).execute().await?;
            for k in &list.keys {
                let _ = kv.delete(&k.name).await;
            }
            Ok::<_, worker::Error>(())
        })
        .await?;

        Ok(ObjectMetadata::new(
            self.key.clone(),
            etag,
            size,
            Some("application/octet-stream".into()),
        ))
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        let prefix = format!("{}:upload:{}:", self.key, self.upload_id);
        let kv = self.kv.clone();
        UnsafeSend(async move {
            let list = kv.list().prefix(prefix).execute().await?;
            for k in &list.keys {
                let _ = kv.delete(&k.name).await;
            }
            Ok::<_, worker::Error>(())
        })
        .await?;

        // Also delete state key
        let state_key = format!("{}:upload:{}:state", self.key, self.upload_id);
        let _ = UnsafeSend(async move { self.kv.delete(&state_key).await }).await;

        Ok(())
    }
}

pub enum FileMultipartUpload {
    R2(R2MultipartUpload),
    Kv(KvMultipartUpload),
}

#[async_trait]
impl pcs_core::types::file_bucket::MultipartUpload for FileMultipartUpload {
    type Error = worker::Error;

    async fn upload_part(
        &mut self,
        part_number: u32,
        data: Vec<u8>,
    ) -> Result<UploadedPart, Self::Error> {
        match self {
            Self::R2(u) => u.upload_part(part_number, data).await,
            Self::Kv(u) => u.upload_part(part_number, data).await,
        }
    }

    async fn complete(&mut self, parts: Vec<UploadedPart>) -> Result<ObjectMetadata, Self::Error> {
        match self {
            Self::R2(u) => u.complete(parts).await,
            Self::Kv(u) => u.complete(parts).await,
        }
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::R2(u) => u.abort().await,
            Self::Kv(u) => u.abort().await,
        }
    }
}

#[async_trait]
impl FileBucket for WorkerBackend {
    type MultipartUpload = FileMultipartUpload;
    type Error = PCSError;

    async fn head(&self, key: &str) -> Result<ObjectMetadata, Self::Error> {
        if self.file_mode == "R2" {
            let bucket = self
                .r2
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("R2 not configured"))?
                .clone();
            let key = key.to_string();
            let obj = UnsafeSend(async move { bucket.head(key).await })
                .await
                .map_err(|e| PCSError::internal_error(e.to_string()))?
                .ok_or_else(|| PCSError::not_found("object not found"))?;
            Ok(ObjectMetadata::new(
                obj.key(),
                obj.http_etag(),
                obj.size(),
                obj.http_metadata().content_type,
            ))
        } else {
            let kv = self
                .file_kv
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("file KV not configured"))?
                .clone();
            let key = key.to_string();
            // Try metadata first
            let meta_key = format!("{}:meta", key);
            let kv_meta = kv.clone();
            let meta: Option<serde_json::Value> = UnsafeSend(async move {
                Ok::<_, worker::Error>(
                    kv_meta
                        .get(&meta_key)
                        .json::<serde_json::Value>()
                        .await
                        .map_err(worker::Error::from)?,
                )
            })
            .await
            .map_err(|e: worker::Error| PCSError::internal_error(e.to_string()))?;

            match meta {
                Some(ref m) => Ok(ObjectMetadata::new(
                    key,
                    m["etag"].as_str().unwrap_or_default().to_string(),
                    m["size"].as_u64().unwrap_or(0),
                    m["content_type"].as_str().map(|s: &str| s.to_string()),
                )),
                None => {
                    // Fallback: read raw file, compute metadata
                    let kv2 = kv.clone();
                    let key2 = key.clone();
                    let data: Option<Vec<u8>> = UnsafeSend(async move {
                        Ok::<_, worker::Error>(
                            kv2.get(&key2).bytes().await.map_err(worker::Error::from)?,
                        )
                    })
                    .await
                    .map_err(|e: worker::Error| PCSError::internal_error(e.to_string()))?;
                    match data {
                        Some(bytes) => {
                            let size = bytes.len() as u64;
                            let etag = kv_object_etag(&bytes);
                            Ok(ObjectMetadata::new(
                                key,
                                etag,
                                size,
                                Some("application/octet-stream".into()),
                            ))
                        }
                        None => Err(PCSError::not_found("object not found")),
                    }
                }
            }
        }
    }

    async fn get_data(&self, key: &str) -> Result<Vec<u8>, Self::Error> {
        if self.file_mode == "R2" {
            let bucket = self
                .r2
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("R2 not configured"))?
                .clone();
            let key = key.to_string();
            let obj = UnsafeSend(async move { bucket.get(key).execute().await })
                .await
                .map_err(|e| PCSError::internal_error(e.to_string()))?
                .ok_or_else(|| PCSError::not_found("object not found"))?;
            let data = if let Some(body) = obj.body() {
                UnsafeSend(async move { body.bytes().await })
                    .await
                    .map_err(|e| PCSError::internal_error(e.to_string()))?
            } else {
                Vec::new()
            };
            Ok(data)
        } else {
            let kv = self
                .file_kv
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("file KV not configured"))?
                .clone();
            let key = key.to_string();
            UnsafeSend(async move {
                Ok::<_, worker::Error>(kv.get(&key).bytes().await.map_err(worker::Error::from)?)
            })
            .await
            .map_err(|e: worker::Error| PCSError::internal_error(e.to_string()))?
            .ok_or_else(|| PCSError::not_found("object not found"))
        }
    }

    async fn delete(&self, key: &str) -> Result<(), Self::Error> {
        if self.file_mode == "R2" {
            let bucket = self
                .r2
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("R2 not configured"))?
                .clone();
            let key = key.to_string();
            UnsafeSend(async move { bucket.delete(key).await })
                .await
                .map_err(|e| PCSError::internal_error(e.to_string()))?;
        } else {
            let kv = self
                .file_kv
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("file KV not configured"))?
                .clone();
            let key = key.to_string();
            let meta_key = format!("{}:meta", key);
            UnsafeSend(async move {
                let _ = kv.delete(&key).await;
                let _ = kv.delete(&meta_key).await;
                Ok::<_, worker::Error>(())
            })
            .await
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        }
        Ok(())
    }

    async fn create_multipart_upload(&self, key: &str) -> Result<String, Self::Error> {
        let upload_id = random_id();
        if self.file_mode == "R2" {
            let bucket = self
                .r2
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("R2 not configured"))?
                .clone();
            let key = key.to_string();
            let upload =
                UnsafeSend(async move { bucket.create_multipart_upload(key).execute().await })
                    .await
                    .map_err(|e| PCSError::internal_error(e.to_string()))?;
            Ok(UnsafeSend(async move { upload.upload_id().await }).await)
        } else {
            let kv = self
                .file_kv
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("file KV not configured"))?;
            let state_key = format!("{}:upload:{}:state", key, upload_id);
            let kv = kv.clone();
            UnsafeSend(async move {
                kv.put(&state_key, serde_json::to_vec(&Vec::<u32>::new())?)?
                    .execute()
                    .await
            })
            .await
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
            Ok(upload_id)
        }
    }

    async fn get_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
    ) -> Result<Self::MultipartUpload, Self::Error> {
        if self.file_mode == "R2" {
            let bucket = self
                .r2
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("R2 not configured"))?;
            let upload = bucket
                .resume_multipart_upload(key, upload_id)
                .map_err(|e| PCSError::internal_error(e.to_string()))?;
            Ok(FileMultipartUpload::R2(R2MultipartUpload {
                upload: Some(upload),
            }))
        } else {
            let kv = self
                .file_kv
                .as_ref()
                .ok_or_else(|| PCSError::internal_error("file KV not configured"))?
                .clone();
            Ok(FileMultipartUpload::Kv(KvMultipartUpload {
                kv,
                key: key.to_string(),
                upload_id: upload_id.to_string(),
            }))
        }
    }
}
