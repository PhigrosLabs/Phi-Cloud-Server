use crate::{
    file::model::FileToken,
    types::{
        FileBucket,
        backend::PCSBackend,
        error::{ErrorCode, PCSError},
        kv::KVStorage,
    },
    utils::MapPCSError,
};

pub async fn get_file_token<B: PCSBackend>(backend: &B, key: &str) -> Result<FileToken, PCSError> {
    let kv = backend.kv();
    kv.get::<FileToken>(key)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .ok_or_else(PCSError::db_not_found)
}

pub async fn save_file_token<B: PCSBackend>(backend: &B, ft: &FileToken) -> Result<(), PCSError> {
    let kv = backend.kv();
    kv.put::<FileToken>(&ft.key, ft)
        .await
        .map_pcs_error(ErrorCode::KV_PUT)?;
    Ok(())
}

pub async fn delete_file_token<B: PCSBackend>(backend: &B, key: &str) -> Result<(), PCSError> {
    let kv = backend.kv();
    let fb = backend.fb();
    fb.delete(key).await.map_pcs_error(ErrorCode::FB_DELETE)?;
    kv.delete::<FileToken>(key)
        .await
        .map_pcs_error(ErrorCode::KV_DELETE)?;
    Ok(())
}

pub async fn get_file_stream<B: PCSBackend>(
    backend: &B,
    key: &str,
) -> Result<<B::FB as FileBucket>::Stream, PCSError> {
    let fb = backend.fb();
    fb.get(key).await.map_pcs_error(ErrorCode::FB_GET)
}
