use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use pcs_core::types::file_bucket::{FileBucket, MultipartUpload, ObjectMetadata, UploadedPart};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

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

#[derive(Clone)]
pub struct FileFileBucket {
    base_path: PathBuf,
}

impl FileFileBucket {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn object_path(&self, key: &str) -> PathBuf {
        self.base_path.join(key)
    }

    fn parts_dir(&self) -> PathBuf {
        self.base_path.join(".parts")
    }

    fn state_path(&self, upload_id: &str) -> PathBuf {
        self.parts_dir().join(format!("{}.json", upload_id))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PartInfo {
    part_number: i32,
    etag: String,
}

impl From<&PartInfo> for UploadedPart {
    fn from(p: &PartInfo) -> Self {
        UploadedPart::new(p.part_number, &p.etag)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct UploadState {
    key: String,
    parts: Vec<PartInfo>,
}

pub struct FileMultipartUpload {
    base_path: PathBuf,
    upload_id: String,
    state: UploadState,
}

impl FileMultipartUpload {
    fn state_path(&self) -> PathBuf {
        self.base_path
            .join(".parts")
            .join(format!("{}.json", self.upload_id))
    }

    fn parts_dir(&self) -> PathBuf {
        self.base_path.join(".parts").join(&self.upload_id)
    }

    async fn save_state(&self) -> Result<(), std::io::Error> {
        let json = serde_json::to_vec(&self.state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(&self.state_path(), &json).await
    }
}

#[async_trait]
impl MultipartUpload for FileMultipartUpload {
    type Error = std::io::Error;

    async fn upload_part(
        &mut self,
        part_number: u32,
        data: Vec<u8>,
    ) -> Result<UploadedPart, Self::Error> {
        let etag = compute_etag(&data);
        let part_path = self.parts_dir().join(part_number.to_string());

        tokio::fs::create_dir_all(self.parts_dir()).await?;

        let mut file = tokio::fs::File::create(&part_path).await?;
        file.write_all(&data).await?;
        file.flush().await?;

        let part = UploadedPart::new(part_number as i32, &etag);
        self.state.parts.push(PartInfo {
            part_number: part_number as i32,
            etag: etag.clone(),
        });
        self.save_state().await?;

        Ok(part)
    }

    async fn complete(&mut self, _parts: Vec<UploadedPart>) -> Result<ObjectMetadata, Self::Error> {
        let object_path = {
            let base = &self.base_path;
            base.join(&self.state.key)
        };

        if let Some(parent) = object_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut final_file = tokio::fs::File::create(&object_path).await?;
        let mut total_size: u64 = 0;
        let mut final_hasher = Sha256::new();

        let mut sorted_parts: Vec<&PartInfo> = self.state.parts.iter().collect();
        sorted_parts.sort_by_key(|p| p.part_number);

        for part in &sorted_parts {
            let part_path = self.parts_dir().join(part.part_number.to_string());
            let data = tokio::fs::read(&part_path).await?;
            final_hasher.update(&data);
            final_file.write_all(&data).await?;
            total_size += data.len() as u64;
        }

        final_file.flush().await?;

        let etag = format!("{:x}", final_hasher.finalize());

        tokio::fs::remove_dir_all(self.parts_dir()).await?;
        let _ = tokio::fs::remove_file(self.state_path()).await;

        Ok(ObjectMetadata::new(&self.state.key, etag, total_size))
    }

    async fn abort(&mut self) -> Result<(), Self::Error> {
        let _ = tokio::fs::remove_dir_all(self.parts_dir()).await;
        let _ = tokio::fs::remove_file(self.state_path()).await;
        Ok(())
    }
}

#[async_trait]
impl FileBucket for FileFileBucket {
    type MultipartUpload = FileMultipartUpload;
    type Error = std::io::Error;

    async fn head(&self, key: impl Into<String> + Send) -> Result<ObjectMetadata, Self::Error> {
        let key = sanitize_key(&key.into())?;
        let path = self.object_path(&key);
        let meta = tokio::fs::metadata(&path).await?;

        if !meta.is_file() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "object not found",
            ));
        }

        let etag = format!(
            "{:x}-{}",
            meta.len(),
            meta.modified()?
                .duration_since(std::time::SystemTime::UNIX_EPOCH,)
                .unwrap_or_default()
                .as_nanos()
        );

        Ok(ObjectMetadata::new(key, etag, meta.len()))
    }

    async fn get(
        &self,
        key: impl Into<String> + Send,
    ) -> Result<impl Stream<Item = Bytes> + Send + Unpin + 'static, Self::Error> {
        use tokio_util::io::ReaderStream;

        let key = sanitize_key(&key.into())?;
        let path = self.object_path(&key);
        let file = tokio::fs::File::open(&path).await?;

        Ok(ReaderStream::new(file).map(|result| match result {
            Ok(bytes) => bytes,
            Err(_) => Bytes::new(),
        }))
    }

    async fn delete(&self, key: impl Into<String> + Send) -> Result<(), Self::Error> {
        let key = sanitize_key(&key.into())?;
        let path = self.object_path(&key);
        tokio::fs::remove_file(&path).await
    }

    async fn create_multipart_upload(
        &self,
        key: impl Into<String> + Send,
    ) -> Result<String, Self::Error> {
        let key = sanitize_key(&key.into())?;
        let upload_id = crate::backend::random_id();

        let parts_dir = self.parts_dir();
        tokio::fs::create_dir_all(&parts_dir).await?;

        let state = UploadState {
            key,
            parts: Vec::new(),
        };

        let json = serde_json::to_vec(&state)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(&self.state_path(&upload_id), &json).await?;

        Ok(upload_id)
    }

    async fn get_multipart_upload(
        &self,
        key: impl Into<String> + Send,
        upload_id: impl Into<String> + Send,
    ) -> Result<Self::MultipartUpload, Self::Error> {
        let key = sanitize_key(&key.into())?;
        let upload_id = upload_id.into();

        let state_data = tokio::fs::read_to_string(&self.state_path(&upload_id)).await?;
        let state: UploadState = serde_json::from_str(&state_data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        if state.key != key {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "upload key mismatch",
            ));
        }

        Ok(FileMultipartUpload {
            base_path: self.base_path.clone(),
            upload_id,
            state,
        })
    }
}
