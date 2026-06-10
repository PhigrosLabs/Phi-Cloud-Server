use alloc::vec::Vec;

use crate::{
    file,
    game::{model::*, types::*, utils::*},
    types::{Date, backend::PCSBackend, error::PCSError, event::Event},
    user,
    utils::ToRfc3339Z,
};

pub async fn handle_create<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    params: GameSaveParams,
) -> Result<PutGameSaveResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;
    let _file_token = file::get_file_token(backend, &params.game_file.object_id).await?;

    let gs = GameSave::new(
        params.summary,
        params.game_file.object_id,
        params.modified_at.iso,
        backend,
    );

    save_game_save(backend, &gs).await?;
    add_game_save_to_user(backend, &session.object_id, &gs.object_id).await?;

    backend
        .emit_event(Event::SaveCreate {
            user: (&session).into(),
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

    let gs_ids = get_game_save_ids_by_user(backend, &session.object_id).await?;
    let mut items = Vec::new();
    for gs_objid in &gs_ids {
        if let Ok(gs) = get_game_save(backend, gs_objid).await {
            let ft = file::get_file_token(backend, &gs.game_file_object_id).await?;
            items.push(GameSaveItem {
                summary: gs.summary,
                game_file: ft.to_response(server_url),
                user: Pointer::new("_User", &session.object_id),
                name: "save".into(),
                modified_at: Date::new(gs.modified_at),
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
    let mut gs = get_game_save(backend, object_id).await?;

    gs.modified_at = params.modified_at.iso;
    gs.summary = params.summary;
    gs.game_file_object_id = params.game_file.object_id;
    gs.updated_at = backend.utc_now();

    save_game_save(backend, &gs).await?;

    backend
        .emit_event(Event::SaveUpdate {
            user: (&session).into(),
            file_object_id: gs.game_file_object_id.clone(),
            summary: gs.summary.clone(),
        })
        .await;

    Ok(PutGameSaveResponse {
        object_id: gs.object_id,
        created_at: gs.created_at.to_rfc3339_z(),
    })
}

pub async fn handle_delete<B: PCSBackend>(
    backend: &B,
    object_id: &str,
    session_token: &str,
) -> Result<(), PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;
    let gs = get_game_save(backend, object_id).await?;

    delete_game_save(backend, object_id).await?;
    remove_game_save_from_user(backend, &session.object_id, object_id).await?;
    file::delete_file_token(backend, &gs.game_file_object_id).await?;

    Ok(())
}
