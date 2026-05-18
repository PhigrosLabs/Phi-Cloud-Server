use alloc::{string::String, vec::Vec};

use crate::{
    file::{self, utils::get_file_token},
    game::{model::GameSave, types::*},
    types::{
        backend::PCSBackend,
        error::PCSError,
        event::Event,
        kv::{KVStorage, KVTable},
    },
    user,
    utils::ToRfc3339Z,
};

use crate::utils::MapPCSError;

pub async fn handle_create<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    params: GameSaveParams,
) -> Result<PutGameSaveResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;
    let _file_token = get_file_token(backend, &params.game_file.object_id).await?;

    let gs = GameSave::new(
        params.summary,
        params.game_file.object_id,
        params.modified_at.iso,
        backend,
    );

    let kv = backend.kv();
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;
    game_saves.put(&gs.object_id, &gs).await.map_db_err()?;

    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let mut list: Vec<String> = games_by_user
        .get(&session.object_id)
        .await
        .map_db_err()?
        .unwrap_or_default();
    list.push(gs.object_id.clone());
    games_by_user
        .put(&session.object_id, &list)
        .await
        .map_internal_err()?;

    let event_user = (&session).into();
    backend
        .emit_event(Event::SaveCreate {
            user: event_user,
            file_object_id: gs.game_file_object_id.clone(),
            summary: gs.summary.clone(),
        })
        .await;

    Ok(PutGameSaveResponse {
        object_id: gs.object_id,
        created_at: gs.created_at.to_rfc3339_z(),
    })
}

pub async fn handle_list<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    server_url: &str,
) -> Result<ListGameSaveResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;

    let kv = backend.kv();
    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;

    let gs_ids: Vec<String> = games_by_user
        .get(&session.object_id)
        .await
        .map_db_err()?
        .unwrap_or_default();
    let mut items = Vec::new();
    for gs_objid in &gs_ids {
        if let Some(gs) = game_saves.get::<GameSave>(gs_objid).await.map_db_err()? {
            let ft = file::get_file_token(backend, &gs.game_file_object_id).await?;
            items.push(GameSaveItem {
                summary: gs.summary,
                game_file: ft.to_response(server_url),
                user: Pointer::new("_User", &session.object_id),
                name: ".save".into(),
                modified_at: GameDate::new(gs.modified_at),
                object_id: gs.object_id.clone(),
                created_at: gs.created_at.to_rfc3339_z(),
                updated_at: gs.updated_at.to_rfc3339_z(),
            });
        }
    }

    Ok(ListGameSaveResponse { results: items })
}

pub async fn handle_update<B: PCSBackend>(
    backend: &B,
    object_id: &str,
    session_token: &str,
    params: GameSaveParams,
) -> Result<PutGameSaveResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;

    let kv = backend.kv();
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;
    let mut gs: GameSave = game_saves
        .get(object_id)
        .await
        .map_db_err()?
        .ok_or_else(PCSError::db_not_found)?;

    gs.summary = params.summary;
    gs.modified_at = params.modified_at.iso;
    gs.game_file_object_id = params.game_file.object_id;
    gs.updated_at = backend.get_utc_now();

    game_saves.put(&gs.object_id, &gs).await.map_db_err()?;

    let event_user = (&session).into();
    backend
        .emit_event(Event::SaveUpdate {
            user: event_user,
            file_object_id: gs.game_file_object_id.clone(),
            summary: gs.summary.clone(),
        })
        .await;

    Ok(PutGameSaveResponse {
        object_id: gs.object_id,
        created_at: gs.created_at.to_rfc3339_z(),
    })
}
