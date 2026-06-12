use alloc::{string::ToString, vec::Vec};

use crate::{
    file::{self, FileTokenInfo},
    game::{model::*, types::*, utils::*},
    types::{Date, ErrorCode, backend::PCSBackend, error::PCSError, event::Event},
    user,
    utils::ToRfc3339Z,
};

pub async fn handle_create<B: PCSBackend>(
    backend: &B,
    session_token: &str,
    params: GameSaveBody,
) -> Result<PutGameSaveResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;
    file::head_file(backend, &params.game_file.object_id).await?;

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
            let ft = FileTokenInfo::get_from_meta_data(
                file::head_file(backend, &gs.game_file_object_id)
                    .await?
                    .ok_or(PCSError::internal_error(
                        ErrorCode::FB_HEAD,
                        "file no found",
                    ))?,
            );
            items.push(GetGameSaveResponse {
                summary: gs.summary,
                game_file: ft.to_lc_file(server_url),
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
    params: GameSaveBody,
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
    file::delete_file(backend, &gs.game_file_object_id).await?;

    Ok(())
}

pub async fn handle_get<B: PCSBackend>(
    backend: &B,
    object_id: &str,
    session_token: &str,
    server_url: &str,
) -> Result<GetGameSaveResponse, PCSError> {
    let session = user::get_session_by_token(backend, session_token).await?;
    let gs_ids = get_game_save_ids_by_user(backend, &session.object_id).await?;
    if let Ok(gs) = get_game_save(backend, object_id).await
        && gs_ids.contains(&object_id.to_string())
    {
        let ft = FileTokenInfo::get_from_meta_data(
            file::head_file(backend, &gs.game_file_object_id)
                .await?
                .ok_or(PCSError::internal_error(
                    ErrorCode::FB_HEAD,
                    "file no found",
                ))?,
        );
        Ok(GetGameSaveResponse {
            summary: gs.summary,
            game_file: ft.to_lc_file(server_url),
            user: Pointer::new("_User", &session.object_id),
            name: "save".into(),
            modified_at: Date::new(gs.modified_at),
            object_id: gs.object_id.clone(),
            created_at: gs.created_at.to_rfc3339_z(),
            updated_at: gs.updated_at.to_rfc3339_z(),
        })
    } else {
        Err(PCSError::not_found(ErrorCode::KV_GET, "GameSave not found"))
    }
}
