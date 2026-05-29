use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use phi_save_codec::{GameKey, GameProgress, GameRecord, Settings, User};

use super::save_provider::SaveProvider;
use crate::types::KVTable;
use crate::utils::{ToRfc3339Z, stream_to_bytes};
use crate::{
    file,
    file::model::{FileToken, MetaData},
    game::model::GameSave,
    types::{backend::PCSBackend, error::PCSError, file_bucket::FileBucket, kv::KVStorage},
    user,
    utils::MapPCSError,
};

#[derive(Debug, Serialize)]
pub struct SaveExtensionResponse {
    pub game_key: GameKey,
    pub game_record: GameRecord,
    pub game_progress: GameProgress,
    pub settings: Settings,
    pub user: User,
    pub name: String,
    pub updated_at: String,
}

pub async fn handle_save_extension_get<B: PCSBackend>(
    backend: &B,
    session_token: &str,
) -> Result<SaveExtensionResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;

    let kv = backend.kv();
    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;

    let gs_ids: Vec<String> = games_by_user
        .get(&session.object_id)
        .await
        .map_db_err()?
        .unwrap_or_default();

    if gs_ids.is_empty() {
        return Err(PCSError::not_found("no game saves found"));
    }

    let last_gs_id = &gs_ids[0];
    let gs: GameSave = game_saves
        .get(last_gs_id)
        .await
        .map_db_err()?
        .ok_or_else(PCSError::db_not_found)?;

    let ft = file::get_file_token(backend, &gs.game_file_object_id).await?;
    let fb = backend.fb();
    let stream = fb.get(&ft.key).await.map_internal_err()?;
    let data = stream_to_bytes(stream).await.map_internal_err()?;

    let provider = SaveProvider::parse(&data)
        .map_err(|e| PCSError::bad_request(alloc::format!("invalid save data: {:?}", e)))?;

    Ok(SaveExtensionResponse {
        game_key: provider
            .get_game_key()
            .map_err(|e| PCSError::internal_error(e.to_string()))?,
        game_record: provider
            .get_game_record()
            .map_err(|e| PCSError::internal_error(e.to_string()))?,

        game_progress: provider
            .get_game_progress()
            .map_err(|e| PCSError::internal_error(e.to_string()))?,
        settings: provider
            .get_settings()
            .map_err(|e| PCSError::internal_error(e.to_string()))?,

        user: provider
            .get_user()
            .map_err(|e| PCSError::internal_error(e.to_string()))?,

        name: session.nickname,
        updated_at: session.updated_at.to_rfc3339_z(),
    })
}

#[derive(Debug, Deserialize)]
pub struct SaveExtensionUpdateRequest {
    pub game_key: Option<GameKey>,
    pub game_record: Option<GameRecord>,
    pub game_progress: Option<GameProgress>,
    pub settings: Option<Settings>,
    pub user: Option<User>,
}

pub async fn handle_save_extension_put<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    body: &[u8],
) -> Result<(), PCSError> {
    let params: SaveExtensionUpdateRequest = serde_json::from_slice(body).map_bad_err()?;

    let session = user::get_session_by_token(backend, session_token).await?;

    let kv = backend.kv();
    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;

    let gs_ids: Vec<String> = games_by_user
        .get(&session.object_id)
        .await
        .map_db_err()?
        .unwrap_or_default();

    if gs_ids.is_empty() {
        return Err(PCSError::not_found("no game saves found"));
    }

    let first_gs_id = &gs_ids[0];
    let mut gs: GameSave = game_saves
        .get(first_gs_id)
        .await
        .map_db_err()?
        .ok_or_else(PCSError::db_not_found)?;

    let ft = file::get_file_token(backend, &gs.game_file_object_id).await?;
    let old_file_key = ft.key.clone();
    let fb = backend.fb();

    let all_fields_provided = params.game_key.is_some()
        && params.game_record.is_some()
        && params.game_progress.is_some()
        && params.settings.is_some()
        && params.user.is_some();

    #[allow(unused_assignments)]
    let mut save_data = None;
    let provider = if all_fields_provided {
        let mut p = SaveProvider::new();
        p.set_game_key(params.game_key.as_ref().unwrap())
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        p.set_game_record(params.game_record.as_ref().unwrap())
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        p.set_game_progress(params.game_progress.as_ref().unwrap())
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        p.set_settings(params.settings.as_ref().unwrap())
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        p.set_user(params.user.as_ref().unwrap())
            .map_err(|e| PCSError::internal_error(e.to_string()))?;
        p
    } else {
        let stream = fb.get(&ft.key).await.map_internal_err()?;
        save_data = Some(stream_to_bytes(stream).await.map_internal_err()?);
        let mut p = SaveProvider::parse(save_data.as_ref().unwrap())
            .map_err(|e| PCSError::bad_request(alloc::format!("invalid save data: {:?}", e)))?;
        if let Some(ref v) = params.game_key {
            p.set_game_key(v)
                .map_err(|e| PCSError::internal_error(e.to_string()))?;
        }
        if let Some(ref v) = params.game_record {
            p.set_game_record(v)
                .map_err(|e| PCSError::internal_error(e.to_string()))?;
        }
        if let Some(ref v) = params.game_progress {
            p.set_game_progress(v)
                .map_err(|e| PCSError::internal_error(e.to_string()))?;
        }
        if let Some(ref v) = params.settings {
            p.set_settings(v)
                .map_err(|e| PCSError::internal_error(e.to_string()))?;
        }
        if let Some(ref v) = params.user {
            p.set_user(v)
                .map_err(|e| PCSError::internal_error(e.to_string()))?;
        }
        p
    };

    let new_data = provider
        .build()
        .map_err(|e| PCSError::internal_error(e.to_string()))?;

    let checksum = {
        use md5::Digest;
        let mut hasher = md5::Md5::new();
        hasher.update(&new_data);
        hex::encode(hasher.finalize())
    };

    let meta_data = MetaData::new(new_data.len() as u64, checksum, ft.meta_data.prefix.clone());
    let new_ft = FileToken::new(meta_data, ft.name.clone(), ft.acl.clone(), backend);
    file::save_file_token(backend, &new_ft).await?;

    fb.put(&new_ft.key, new_data).await.map_internal_err()?;

    let utc_now = backend.utc_now();
    gs.modified_at = utc_now.to_rfc3339_z();
    gs.game_file_object_id = new_ft.key;
    gs.updated_at = utc_now;
    game_saves.put(&gs.object_id, &gs).await.map_db_err()?;

    let _ = fb.delete(&old_file_key).await;
    let file_tokens = kv.open_table("file_tokens").await.map_db_err()?;
    let _ = file_tokens.delete(&old_file_key).await;

    Ok(())
}
