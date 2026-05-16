use async_trait::async_trait;
use std::error::Error;
use std::fmt::Debug;

#[derive(Debug)]
pub struct UploadedPart {
    pub part_number: i32,
    pub etag: String,
}

impl UploadedPart {
    pub fn new(part_number: i32, etag: impl Into<String>) -> Self {
        Self {
            part_number,
            etag: etag.into(),
        }
    }
}

#[derive(Debug)]
pub struct ObjectMetadata {
    pub key: String,
    pub etag: String,
    pub size: u64,
    pub content_type: Option<String>,
}

impl ObjectMetadata {
    pub fn new(
        key: impl Into<String>,
        etag: impl Into<String>,
        size: u64,
        content_type: Option<String>,
    ) -> Self {
        Self {
            key: key.into(),
            etag: etag.into(),
            size,
            content_type,
        }
    }
}

#[async_trait]
pub trait MultipartUpload: Send + Sync {
    type Error: Error + Send;

    async fn upload_part(
        &mut self,
        part_number: u32,
        data: Vec<u8>,
    ) -> Result<UploadedPart, Self::Error>;

    async fn complete(&mut self, parts: Vec<UploadedPart>) -> Result<ObjectMetadata, Self::Error>;

    async fn abort(&mut self) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait FileBucket: Sync + Send + 'static {
    type MultipartUpload: MultipartUpload;
    type Error: Error + Send;

    async fn head(&self, key: &str) -> Result<ObjectMetadata, Self::Error>;

    async fn get_data(&self, key: &str) -> Result<Vec<u8>, Self::Error>;

    async fn delete(&self, key: &str) -> Result<(), Self::Error>;

    async fn create_multipart_upload(&self, key: &str) -> Result<String, Self::Error>;

    async fn get_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
    ) -> Result<Self::MultipartUpload, Self::Error>;
}
