use crate::{
    types::{ErrorCode, FileBucket, Metadata, ObjectMetadata, PCSBackend, PCSError},
    utils::MapPCSError,
};

pub async fn get_file<B: PCSBackend>(
    backend: &B,
    key: &str,
) -> Result<Option<(ObjectMetadata, <B::FB as FileBucket>::Stream)>, PCSError> {
    backend.fb().get(key).await.map_pcs_error(ErrorCode::FB_GET)
}

pub async fn head_file<B: PCSBackend>(
    backend: &B,
    key: &str,
) -> Result<Option<ObjectMetadata>, PCSError> {
    backend
        .fb()
        .head(key)
        .await
        .map_pcs_error(ErrorCode::FB_HEAD)
}

pub async fn delete_file<B: PCSBackend>(backend: &B, key: &str) -> Result<(), PCSError> {
    backend
        .fb()
        .delete(key)
        .await
        .map_pcs_error(ErrorCode::FB_DELETE)
}

pub async fn put_file<B: PCSBackend>(
    backend: &B,
    key: &str,
    data: &[u8],
    custom_metadata: Metadata,
) -> Result<(), PCSError> {
    backend
        .fb()
        .put(key, data, custom_metadata)
        .await
        .map_pcs_error(ErrorCode::FB_DELETE)?;
    Ok(())
}
