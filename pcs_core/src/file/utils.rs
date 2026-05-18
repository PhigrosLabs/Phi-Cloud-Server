use crate::{
    file::model::FileToken,
    types::{
        backend::PCSBackend,
        error::PCSError,
        kv::{KVStorage, KVTable},
    },
    utils::MapPCSError,
};

pub async fn get_file_token<B: PCSBackend>(backend: &B, key: &str) -> Result<FileToken, PCSError> {
    let kv = backend.kv();
    let file_tokens = kv.open_table("file_tokens").await.map_db_err()?;
    file_tokens
        .get(key)
        .await
        .map_db_err()?
        .ok_or_else(PCSError::db_not_found)
}

pub async fn save_file_token<B: PCSBackend>(backend: &B, ft: &FileToken) -> Result<(), PCSError> {
    let kv = backend.kv();
    let file_tokens = kv.open_table("file_tokens").await.map_db_err()?;
    file_tokens.put(&ft.key, ft).await.map_db_err()?;
    Ok(())
}
