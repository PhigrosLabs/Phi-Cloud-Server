use alloc::{string::ToString, vec::Vec};

use crate::{
    file, game,
    types::{
        backend::PCSBackend,
        error::{ErrorCode, PCSError},
        event::{Event, EventUser},
        kv::KVStorage,
    },
    user::{
        AuthData, SessionResponse, UpdateUserParams, UserResponse, delete_session,
        get_session_by_object_id, get_session_by_token, get_session_token_by_openid,
        model::Session, put_session_with_indices, save_session,
    },
    utils::{MapPCSError, ToRfc3339Z},
};

pub async fn handle_register<B: PCSBackend>(
    backend: &B,
    auth: AuthData,
) -> Result<SessionResponse, PCSError> {
    let check_result = backend.user_check(&auth).await?;
    let name = check_result.name.unwrap_or(auth.name);
    let short_id = check_result.short_id.unwrap_or_else(|| "PCS".to_string());

    if let Some(token) = get_session_token_by_openid(backend, &auth.openid).await? {
        let session = get_session_by_token(backend, &token).await?;
        backend
            .emit_event(Event::UserLogin {
                user: EventUser::from(&session),
            })
            .await;
        return Ok(SessionResponse::from(&session));
    }

    let session = Session::new(name, auth.openid, short_id, backend);
    put_session_with_indices(backend, &session).await?;
    backend
        .emit_event(Event::UserCreate {
            user: EventUser::from(&session),
        })
        .await;
    Ok(SessionResponse::from(&session))
}

pub async fn handle_get_current<B: PCSBackend>(
    backend: &B,
    session_token: &str,
) -> Result<UserResponse, PCSError> {
    let session = get_session_by_token(backend, session_token).await?;
    Ok(UserResponse {
        object_id: session.object_id,
        nickname: session.nickname,
        created_at: session.created_at.to_rfc3339_z(),
        updated_at: session.updated_at.to_rfc3339_z(),
    })
}

pub async fn handle_update<B: PCSBackend>(
    backend: &B,
    object_id: &str,
    params: UpdateUserParams,
) -> Result<SessionResponse, PCSError> {
    let mut session = get_session_by_object_id(backend, object_id).await?;
    let old = EventUser::from(&session);
    session.nickname = params.nickname;
    session.updated_at = backend.utc_now();
    save_session(backend, &session).await?;
    backend
        .emit_event(Event::UserUpdate {
            user: old,
            new: EventUser::from(&session),
        })
        .await;
    Ok(SessionResponse::from(&session))
}

pub async fn handle_delete<B: PCSBackend>(
    backend: &B,
    object_id: &str,
    session_token: &str,
) -> Result<(), PCSError> {
    let session = get_session_by_token(backend, session_token).await?;
    if session.object_id != object_id {
        return Err(PCSError::unauthorized(
            ErrorCode::UNAUTHORIZED_DELETE_USER,
            "not authorized to delete this user",
        ));
    }

    let gs_ids = game::get_game_save_ids_by_user(backend, &session.object_id).await?;
    for gs_objid in &gs_ids {
        if let Ok(gs) = game::get_game_save(backend, gs_objid).await {
            file::utils::delete_file_token(backend, &gs.game_file_object_id).await?;
        }
        game::delete_game_save(backend, gs_objid).await?;
    }
    game::put_game_save_ids_by_user(backend, &session.object_id, Vec::new()).await?;

    delete_session(backend, &session).await?;
    backend
        .emit_event(Event::UserDelete {
            user: EventUser::from(&session),
        })
        .await;
    Ok(())
}

pub async fn handle_refresh_token<B: PCSBackend>(
    backend: &B,
    object_id: &str,
    session_token: &str,
) -> Result<SessionResponse, PCSError> {
    let mut session = get_session_by_token(backend, session_token).await?;
    if session.object_id != object_id {
        return Err(PCSError::unauthorized(
            ErrorCode::UNAUTHORIZED_REFRESH_SESSION,
            "not authorized to refresh this session",
        ));
    }

    let old_event_user = EventUser::from(&session);

    // 先删除旧 session 记录（索引由 put_session_with_indices 覆写，只删主记录）
    let kv = backend.kv();
    kv.delete::<Session>(&session.session_token)
        .await
        .map_pcs_error(ErrorCode::KV_DELETE)?;

    session.session_token = backend.random_id();
    session.updated_at = backend.utc_now();
    put_session_with_indices(backend, &session).await?;

    backend
        .emit_event(Event::UserRefreshSessionToken {
            user: old_event_user,
            new: EventUser::from(&session),
        })
        .await;
    Ok(SessionResponse::from(&session))
}
