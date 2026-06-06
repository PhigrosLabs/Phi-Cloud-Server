use crate::{
    file::model::FileToken,
    types::{
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
