use chrono::Utc;
use pcs_core::types::file_bucket::{
    FileBucket, Metadata, MultipartUpload, ObjectMetadata, UploadedPart,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

fn compute_etag(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn sanitize_key(key: &str) -> Result<String, std::io::Error> {
    let key = key.trim_start_matches('/');
    if key.contains("..") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid key: path traversal detected",
        ));
    }
    Ok(key.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredMeta {
    etag: String,
    size: u64,
    update_at: chrono::DateTime<Utc>,
    custom_metadata: Metadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    upload_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    parts: Vec<PartInfo>,
}

impl StoredMeta {
    fn to_object_metadata(&self, key: &str) -> ObjectMetadata {
        ObjectMetadata {
            key: key.to_string(),
            etag: self.etag.clone(),
            size: self.size,
            update_at: self.update_at,
            custom_metadata: self.custom_metadata.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PartInfo {
    part_number: u16,
    etag: String,
}

#[derive(Clone)]
pub struct LocalFileBucket {
    base_path: PathBuf,
}

impl LocalFileBucket {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn key_dir(&self, key: &str) -> PathBuf {
        self.base_path.join(key)
    }

    fn meta_path(&self, key: &str) -> PathBuf {
        self.key_dir(key).join("meta")
    }

    fn data_path(&self, key: &str) -> PathBuf {
        self.key_dir(key).join("data")
    }

    async fn read_meta(&self, key: &str) -> Result<Option<StoredMeta>, std::io::Error> {
        let path = self.meta_path(key);
        match tokio::fs::read_to_string(&path).await {
            Ok(json) => {
                let meta: StoredMeta = serde_json::from_str(&json)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                Ok(Some(meta))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn write_meta(&self, key: &str, meta: &StoredMeta) -> Result<(), std::io::Error> {
        let path = self.meta_path(key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_vec(meta)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(&path, &json).await
    }
}

pub struct FileMultipartUpload {
    base_path: PathBuf,
    key: String,
    upload_id: String,
    parts: Vec<PartInfo>,
}

impl FileMultipartUpload {
    fn key_dir(&self) -> PathBuf {
        self.base_path.join(&self.key)
    }

    fn part_path(&self, part_number: u16) -> PathBuf {
        self.key_dir().join("part").join(part_number.to_string())
    }

    fn meta_path(&self) -> PathBuf {
        self.key_dir().join("meta")
    }

    async fn save_meta(&self) -> Result<(), std::io::Error> {
        let json = tokio::fs::read_to_string(&self.meta_path()).await?;
        let mut meta: StoredMeta = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        meta.upload_id = Some(self.upload_id.clone());
        meta.parts = self.parts.clone();
        let json = serde_json::to_vec(&meta)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(&self.meta_path(), &json).await
    }
}

impl MultipartUpload for FileMultipartUpload {
    type Error = std::io::Error;

    async fn upload_part(
        &mut self,
        part_number: u16,
        data: &[u8],
    ) -> Result<UploadedPart, Self::Error> {
        let etag = compute_etag(data);
        let path = self.part_path(part_number);

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = tokio::fs::File::create(&path).await?;
        file.write_all(data).await?;
        file.flush().await?;

        self.parts.push(PartInfo {
            part_number,
            etag: etag.clone(),
        });
        self.save_meta().await?;

        Ok(UploadedPart::new(part_number, &etag))
    }

    async fn complete(&mut self, _parts: Vec<UploadedPart>) -> Result<ObjectMetadata, Self::Error> {
        let data_path = self.key_dir().join("data");

        if let Some(parent) = data_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut final_file = tokio::fs::File::create(&data_path).await?;
        let mut total_size: u64 = 0;
        let mut final_hasher = Sha256::new();

        let mut sorted_parts: Vec<&PartInfo> = self.parts.iter().collect();
        sorted_parts.sort_by_key(|p| p.part_number);

        for part in &sorted_parts {
            let part_path = self.part_path(part.part_number);
            let data = tokio::fs::read(&part_path).await?;
            final_hasher.update(&data);
            final_file.write_all(&data).await?;
            total_size += data.len() as u64;
        }

        final_file.flush().await?;

        let etag = format!("{:x}", final_hasher.finalize());
        let update_at = Utc::now();

        let json = tokio::fs::read_to_string(&self.meta_path()).await?;
        let mut meta: StoredMeta = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        meta.etag = etag.clone();
        meta.size = total_size;
        meta.update_at = update_at;
        meta.upload_id = None;
        meta.parts.clear();

        let json = serde_json::to_vec(&meta)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(&self.meta_path(), &json).await?;

        let _ = tokio::fs::remove_dir_all(self.key_dir().join("part")).await;

        Ok(ObjectMetadata {
            key: self.key.clone(),
            etag,
            size: total_size,
            update_at,
            custom_metadata: meta.custom_metadata,
        })
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        let _ = tokio::fs::remove_dir_all(self.key_dir().join("part")).await;
        if let Ok(json) = tokio::fs::read_to_string(&self.meta_path()).await
            && let Ok(mut meta) = serde_json::from_str::<StoredMeta>(&json)
        {
            meta.upload_id = None;
            meta.parts.clear();
            let json = serde_json::to_vec(&meta).unwrap_or_default();
            let _ = tokio::fs::write(&self.meta_path(), &json).await;
        }
        Ok(())
    }
}

fn map_chunk(result: Result<bytes::Bytes, std::io::Error>) -> Result<Vec<u8>, std::io::Error> {
    result.map(|b| b.to_vec())
}

impl FileBucket for LocalFileBucket {
    type MultipartUpload = FileMultipartUpload;
    type Error = std::io::Error;
    type Stream = futures::stream::Map<
        ReaderStream<tokio::fs::File>,
        fn(Result<bytes::Bytes, std::io::Error>) -> Result<Vec<u8>, std::io::Error>,
    >;

    async fn head(&self, key: &str) -> Result<Option<ObjectMetadata>, Self::Error> {
        let key = sanitize_key(key)?;
        let meta = self.read_meta(&key).await?;
        Ok(meta.map(|m| m.to_object_metadata(&key)))
    }

    async fn get(&self, key: &str) -> Result<Option<(ObjectMetadata, Self::Stream)>, Self::Error> {
        use futures::StreamExt;

        let key = sanitize_key(key)?;
        let meta = match self.read_meta(&key).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        let data_path = self.data_path(&key);
        let file = match tokio::fs::File::open(&data_path).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };

        let stream = ReaderStream::new(file).map(
            map_chunk
                as fn(Result<bytes::Bytes, std::io::Error>) -> Result<Vec<u8>, std::io::Error>,
        );

        Ok(Some((meta.to_object_metadata(&key), stream)))
    }

    async fn delete(&self, key: &str) -> Result<(), Self::Error> {
        let key = sanitize_key(key)?;
        let dir = self.key_dir(&key);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn create_multipart_upload(
        &self,
        key: &str,
        custom_metadata: Metadata,
    ) -> Result<String, Self::Error> {
        let key = sanitize_key(key)?;
        let upload_id = crate::backend::random_id();
        let update_at = Utc::now();

        let key_dir = self.key_dir(&key);
        tokio::fs::create_dir_all(&key_dir).await?;

        let meta = StoredMeta {
            etag: String::new(),
            size: 0,
            update_at,
            custom_metadata,
            upload_id: Some(upload_id.clone()),
            parts: Vec::new(),
        };

        self.write_meta(&key, &meta).await?;

        Ok(upload_id)
    }

    async fn get_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
    ) -> Result<Self::MultipartUpload, Self::Error> {
        let key = sanitize_key(key)?;
        let meta = self.read_meta(&key).await?.ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "upload not found",
        ))?;

        match &meta.upload_id {
            Some(id) if id == upload_id => {}
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "upload id mismatch or upload not in progress",
                ));
            }
        }

        Ok(FileMultipartUpload {
            base_path: self.base_path.clone(),
            key,
            upload_id: upload_id.to_string(),
            parts: meta.parts,
        })
    }

    async fn put(
        &self,
        key: &str,
        data: &[u8],
        custom_metadata: Metadata,
    ) -> Result<ObjectMetadata, Self::Error> {
        let key = sanitize_key(key)?;
        let etag = compute_etag(data);
        let size = data.len() as u64;
        let update_at = Utc::now();

        let key_dir = self.key_dir(&key);
        tokio::fs::create_dir_all(&key_dir).await?;
        tokio::fs::write(self.data_path(&key), data).await?;

        let meta = StoredMeta {
            etag: etag.clone(),
            size,
            update_at,
            custom_metadata: custom_metadata.clone(),
            upload_id: None,
            parts: Vec::new(),
        };
        self.write_meta(&key, &meta).await?;

        Ok(ObjectMetadata {
            key,
            etag,
            size,
            update_at,
            custom_metadata,
        })
    }
}
