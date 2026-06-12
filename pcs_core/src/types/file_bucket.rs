use alloc::string::String;
use alloc::vec::Vec;
use chrono::{DateTime, Utc};
use core::error::Error;
use core::fmt::Debug;
use trait_variant::make;

use crate::types::ByteStream;

#[derive(Debug)]
pub struct UploadedPart {
    pub part_number: u16,
    pub etag: String,
}

impl UploadedPart {
    pub fn new(part_number: u16, etag: impl Into<String>) -> Self {
        Self {
            part_number,
            etag: etag.into(),
        }
    }
}

pub type Metadata = Vec<(String, String)>;

#[derive(Debug)]
pub struct ObjectMetadata {
    pub key: String,
    pub etag: String,
    pub size: u64,
    pub update_at: DateTime<Utc>,
    pub custom_metadata: Metadata,
}

#[make(Send)]
pub trait MultipartUpload: Send + Sync {
    type Error: Error + Send;

    async fn upload_part(
        &mut self,
        part_number: u16,
        data: &[u8],
    ) -> Result<UploadedPart, Self::Error>;

    async fn complete(&mut self, parts: Vec<UploadedPart>) -> Result<ObjectMetadata, Self::Error>;

    async fn abort(&mut self) -> Result<(), Self::Error>;
}

#[make(Send)]
pub trait FileBucket: Sync + Send + 'static {
    type MultipartUpload: MultipartUpload;
    type Error: Error + Send;
    type Stream: ByteStream;

    async fn head(&self, key: &str) -> Result<Option<ObjectMetadata>, Self::Error>;

    async fn get(&self, key: &str) -> Result<Option<(ObjectMetadata, Self::Stream)>, Self::Error>;

    async fn delete(&self, key: &str) -> Result<(), Self::Error>;

    async fn create_multipart_upload(
        &self,
        key: &str,
        custom_metadata: Metadata,
    ) -> Result<String, Self::Error>;

    async fn get_multipart_upload(
        &self,
        key: &str,
        upload_id: &str,
    ) -> Result<Self::MultipartUpload, Self::Error>;

    async fn put(
        &self,
        key: &str,
        data: &[u8],
        custom_metadata: Metadata,
    ) -> Result<ObjectMetadata, Self::Error>;
}
