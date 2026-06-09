use alloc::{string::String, vec::Vec};

use crate::{
    game::model::{GameSave, GameSaveIdsByUser},
    types::{
        backend::PCSBackend,
        error::{ErrorCode, PCSError},
        kv::KVStorage,
    },
    utils::MapPCSError,
};

pub async fn get_game_save<B: PCSBackend>(
    backend: &B,
    object_id: &str,
) -> Result<GameSave, PCSError> {
    let kv = backend.kv();
    kv.get::<GameSave>(object_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .ok_or_else(PCSError::db_not_found)
}

pub async fn save_game_save<B: PCSBackend>(backend: &B, gs: &GameSave) -> Result<(), PCSError> {
    let kv = backend.kv();
    kv.put::<GameSave>(&gs.object_id, gs)
        .await
        .map_pcs_error(ErrorCode::KV_PUT)
}

pub async fn delete_game_save<B: PCSBackend>(backend: &B, object_id: &str) -> Result<(), PCSError> {
    let kv = backend.kv();
    kv.delete::<GameSave>(object_id)
        .await
        .map_pcs_error(ErrorCode::KV_DELETE)
}

pub async fn get_game_save_ids_by_user<B: PCSBackend>(
    backend: &B,
    user_obj_id: &str,
) -> Result<Vec<String>, PCSError> {
    let kv = backend.kv();
    let GameSaveIdsByUser(ids) = kv
        .get::<GameSaveIdsByUser>(user_obj_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .unwrap_or(GameSaveIdsByUser(Vec::new()));
    Ok(ids)
}

pub async fn put_game_save_ids_by_user<B: PCSBackend>(
    backend: &B,
    user_obj_id: &str,
    ids: Vec<String>,
) -> Result<(), PCSError> {
    let kv = backend.kv();
    kv.put::<GameSaveIdsByUser>(user_obj_id, &GameSaveIdsByUser(ids))
        .await
        .map_pcs_error(ErrorCode::KV_PUT)
}

pub async fn add_game_save_to_user<B: PCSBackend>(
    backend: &B,
    user_obj_id: &str,
    gs_obj_id: &str,
) -> Result<(), PCSError> {
    let mut ids = get_game_save_ids_by_user(backend, user_obj_id).await?;
    ids.push(gs_obj_id.into());
    put_game_save_ids_by_user(backend, user_obj_id, ids).await
}

pub async fn remove_game_save_from_user<B: PCSBackend>(
    backend: &B,
    user_obj_id: &str,
    gs_obj_id: &str,
) -> Result<(), PCSError> {
    let mut ids = get_game_save_ids_by_user(backend, user_obj_id).await?;
    ids.retain(|id| id != gs_obj_id);
    put_game_save_ids_by_user(backend, user_obj_id, ids).await
}
