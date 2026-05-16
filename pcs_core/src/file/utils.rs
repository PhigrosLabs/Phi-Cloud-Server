use crate::{
    file::model::FileToken,
    types::{
        backend::PCSBackend,
        error::PCSError,
        kv::KVStorage,
    },
    utils::{MapPCSError, kv_get, kv_put},
};

pub async fn get_file_token<B: PCSBackend>(
    backend: &B,
    object_id: &str,
) -> Result<FileToken, PCSError> {
    let kv = backend.kv().await;
    let file_tokens = kv.open_table("file_tokens").await.map_db_err()?;
    kv_get(&file_tokens, object_id)
        .await?
        .ok_or_else(PCSError::db_not_found)
}

pub async fn save_file_token<B: PCSBackend>(backend: &B, ft: &FileToken) -> Result<(), PCSError> {
    let kv = backend.kv().await;
    let file_tokens = kv.open_table("file_tokens").await.map_db_err()?;
    kv_put(&file_tokens, &ft.object_id, ft).await?;
    Ok(())
}
