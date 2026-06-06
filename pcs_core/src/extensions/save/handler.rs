use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use phi_save_codec::{GameKey, GameProgress, GameRecord, Settings, User};

use super::save_provider::SaveProvider;
use crate::utils::{ToRfc3339Z, stream_to_bytes};
use crate::{
    file,
    file::model::{FileToken, MetaData},
    game::model::{GameSave, GameSaveIdsByUser},
    types::{
        backend::PCSBackend,
        error::{ErrorCode, PCSError},
        file_bucket::FileBucket,
        kv::KVStorage,
    },
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
    let GameSaveIdsByUser(gs_ids) = kv
        .get::<GameSaveIdsByUser>(&session.object_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .unwrap_or(GameSaveIdsByUser(Vec::new()));

    if gs_ids.is_empty() {
        return Err(PCSError::not_found(
            ErrorCode::SAVE_NO_GAME_SAVES_FOUND,
            "no game saves found",
        ));
    }

    let last_gs_id = &gs_ids[0];
    let gs: GameSave = kv
        .get::<GameSave>(last_gs_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .ok_or_else(PCSError::db_not_found)?;

    let ft = file::get_file_token(backend, &gs.game_file_object_id).await?;
    let fb = backend.fb();
    let stream = fb.get(&ft.key).await.map_pcs_error(ErrorCode::FB_GET)?;
    let data = stream_to_bytes(stream)
        .await
        .map_pcs_error(ErrorCode::FB_GET)?;

    let provider = SaveProvider::parse(&data).map_err(|e| {
        PCSError::bad_request(
            ErrorCode::SAVE_INVALID_SAVE_DATA,
            alloc::format!("invalid save data: {:?}", e),
        )
    })?;

    Ok(SaveExtensionResponse {
        game_key: provider
            .get_game_key()
            .map_err(|e| PCSError::internal_error(ErrorCode::SAVE_GET_GAME_KEY, e.to_string()))?,
        game_record: provider.get_game_record().map_err(|e| {
            PCSError::internal_error(ErrorCode::SAVE_GET_GAME_RECORD, e.to_string())
        })?,

        game_progress: provider.get_game_progress().map_err(|e| {
            PCSError::internal_error(ErrorCode::SAVE_GET_GAME_PROGRESS, e.to_string())
        })?,
        settings: provider
            .get_settings()
            .map_err(|e| PCSError::internal_error(ErrorCode::SAVE_GET_SETTINGS, e.to_string()))?,

        user: provider
            .get_user()
            .map_err(|e| PCSError::internal_error(ErrorCode::SAVE_GET_USER, e.to_string()))?,

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
    let params: SaveExtensionUpdateRequest =
        serde_json::from_slice(body).map_pcs_bad(ErrorCode::JSON_DESERIALIZE)?;

    let session = user::get_session_by_token(backend, session_token).await?;

    let kv = backend.kv();
    let GameSaveIdsByUser(gs_ids) = kv
        .get(&session.object_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .unwrap_or(GameSaveIdsByUser(Vec::new()));

    if gs_ids.is_empty() {
        return Err(PCSError::not_found(
            ErrorCode::SAVE_NO_GAME_SAVES_FOUND2,
            "no game saves found",
        ));
    }

    let first_gs_id = &gs_ids[0];
    let mut gs: GameSave = kv
        .get::<GameSave>(first_gs_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
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
            .map_err(|e| PCSError::internal_error(ErrorCode::SAVE_SET_GAME_KEY, e.to_string()))?;
        p.set_game_record(params.game_record.as_ref().unwrap())
            .map_err(|e| {
                PCSError::internal_error(ErrorCode::SAVE_SET_GAME_RECORD, e.to_string())
            })?;
        p.set_game_progress(params.game_progress.as_ref().unwrap())
            .map_err(|e| {
                PCSError::internal_error(ErrorCode::SAVE_SET_GAME_PROGRESS, e.to_string())
            })?;
        p.set_settings(params.settings.as_ref().unwrap())
            .map_err(|e| PCSError::internal_error(ErrorCode::SAVE_SET_SETTINGS, e.to_string()))?;
        p.set_user(params.user.as_ref().unwrap())
            .map_err(|e| PCSError::internal_error(ErrorCode::SAVE_SET_USER, e.to_string()))?;
        p
    } else {
        let stream = fb.get(&ft.key).await.map_pcs_error(ErrorCode::FB_GET)?;
        save_data = Some(
            stream_to_bytes(stream)
                .await
                .map_pcs_error(ErrorCode::FB_GET)?,
        );
        let mut p = SaveProvider::parse(save_data.as_ref().unwrap()).map_err(|e| {
            PCSError::bad_request(
                ErrorCode::SAVE_INVALID_SAVE_DATA2,
                alloc::format!("invalid save data: {:?}", e),
            )
        })?;
        if let Some(ref v) = params.game_key {
            p.set_game_key(v).map_err(|e| {
                PCSError::internal_error(ErrorCode::SAVE_SET_GAME_KEY, e.to_string())
            })?;
        }
        if let Some(ref v) = params.game_record {
            p.set_game_record(v).map_err(|e| {
                PCSError::internal_error(ErrorCode::SAVE_SET_GAME_RECORD, e.to_string())
            })?;
        }
        if let Some(ref v) = params.game_progress {
            p.set_game_progress(v).map_err(|e| {
                PCSError::internal_error(ErrorCode::SAVE_SET_GAME_PROGRESS, e.to_string())
            })?;
        }
        if let Some(ref v) = params.settings {
            p.set_settings(v).map_err(|e| {
                PCSError::internal_error(ErrorCode::SAVE_SET_SETTINGS, e.to_string())
            })?;
        }
        if let Some(ref v) = params.user {
            p.set_user(v)
                .map_err(|e| PCSError::internal_error(ErrorCode::SAVE_SET_USER, e.to_string()))?;
        }
        p
    };

    let new_data = provider
        .build()
        .map_err(|e| PCSError::internal_error(ErrorCode::SAVE_BUILD, e.to_string()))?;

    let checksum = {
        use md5::Digest;
        let mut hasher = md5::Md5::new();
        hasher.update(&new_data);
        hex::encode(hasher.finalize())
    };

    let meta_data = MetaData::new(new_data.len() as u64, checksum, ft.meta_data.prefix.clone());
    let new_ft = FileToken::new(meta_data, ft.name.clone(), ft.acl.clone(), backend);
    file::save_file_token(backend, &new_ft).await?;

    fb.put(&new_ft.key, &new_data)
        .await
        .map_pcs_error(ErrorCode::FB_PUT)?;

    let utc_now = backend.utc_now();
    gs.modified_at = utc_now.to_rfc3339_z();
    gs.game_file_object_id = new_ft.key;
    gs.updated_at = utc_now;
    kv.put::<GameSave>(&gs.object_id, &gs)
        .await
        .map_pcs_error(ErrorCode::KV_PUT)?;

    let _ = fb.delete(&old_file_key).await;
    let _ = kv.delete::<FileToken>(&old_file_key).await;

    Ok(())
}
