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

use crate::utils::{MapPCSError, kv_get, kv_put};

pub async fn handle_create<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    params: CreateGameSaveParams,
) -> Result<CreateGameSaveResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;
    let _file_token = get_file_token(backend, &params.game_file.object_id).await?;

    let gs = GameSave::new(
        params.summary,
        params.game_file.object_id,
        params.modified_at.iso,
        params.name,
        &session.object_id,
        backend,
    );

    let kv = backend.kv().await;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;
    kv_put(&game_saves, &gs.object_id, &gs).await?;

    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let user_gk = format!("{}:{}", session.object_id, gs.object_id);
    games_by_user.put(&user_gk, &[]).await.map_internal_err()?;

    let event_user = (&session).into();
    backend
        .emit_event(Event::SaveCreate {
            user: event_user,
            file_object_id: gs.game_file_object_id.clone(),
            summary: gs.summary.clone(),
        })
        .await;

    Ok(CreateGameSaveResponse {
        object_id: gs.object_id,
        created_at: gs.created_at.to_rfc3339_z(),
    })
}

pub async fn handle_list<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    server_url:&str,
) -> Result<ListGameSaveResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;

    let kv = backend.kv().await;
    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;

    let prefix = format!("{}:", session.object_id);
    let mut items = Vec::new();
    let keys = games_by_user.list_keys(&prefix).await.map_internal_err()?;
    for key in &keys {
        if let Some(gs_objid) = key.strip_prefix(&prefix) {
            if let Some(gs) = kv_get::<GameSave, _>(&game_saves, gs_objid).await? {
                let ft = file::get_file_token(backend, &gs.game_file_object_id).await?;
                items.push(GameSaveItem {
                    summary: gs.summary,
                    game_file: ft.to_response(server_url),
                    user: Pointer::new("_User", &gs.user_object_id),
                    modified_at: GameDate::new(gs.modified_at),
                    name: gs.name,
                    object_id: gs.object_id,
                    created_at: gs.created_at.to_rfc3339_z(),
                    updated_at: gs.updated_at.to_rfc3339_z(),
                });
            }
        }
    }

    Ok(ListGameSaveResponse {results:items})
}

pub async fn handle_update<B: PCSBackend>(
    backend: &B,
    object_id: &str,
    session_token: &str,
    params: UpdateGameSaveParams,
) -> Result<(), PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;

    let kv = backend.kv().await;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;
    let mut gs: GameSave = kv_get(&game_saves, object_id)
        .await?
        .ok_or_else(PCSError::db_not_found)?;

    if gs.user_object_id != session.object_id {
        return Err(PCSError::unauthorized(
            "not authorized to update this game save",
        ));
    }

    gs.summary = params.summary;
    gs.name = params.name;
    gs.modified_at = params.modified_at.iso;
    gs.updated_at = backend.get_utc_now();

    kv_put(&game_saves, &gs.object_id, &gs).await?;

    let event_user = (&session).into();
    backend
        .emit_event(Event::SaveUpdate {
            user: event_user,
            file_object_id: gs.game_file_object_id.clone(),
            summary: gs.summary.clone(),
        })
        .await;

    Ok(())
}
