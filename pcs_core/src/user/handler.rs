use crate::{
    file,
    game::model::GameSave,
    types::{
        backend::PCSBackend,
        error::PCSError,
        event::{Event, EventUser},
        kv::{KVStorage, KVTable},
    },
    user::{
        AuthData, SessionResponse, UpdateUserParams, UserResponse, delete_session_tables,
        get_session_by_object_id, get_session_by_token, model::Session, save_session,
    },
    utils::ToRfc3339Z,
};

use crate::utils::{MapPCSError, kv_delete, kv_get, kv_put};

pub async fn handle_register<B: PCSBackend>(
    backend: &B,
    auth: AuthData,
) -> Result<SessionResponse, PCSError> {
    let check_result = backend.user_check(&auth).await?;
    let name = check_result.name.unwrap_or(auth.name);
    let short_id = check_result.short_id.unwrap_or_else(|| "PCS".to_string());

    let kv = backend.kv().await;
    let sessions_by_openid = kv.open_table("sessions_by_openid").await.map_db_err()?;

    if let Some(token) = kv_get::<String, _>(&sessions_by_openid, &auth.openid).await? {
        let session = get_session_by_token(backend, &token).await?;
        backend
            .emit_event(Event::UserLogin {
                user: EventUser::from(&session),
            })
            .await;
        return Ok(SessionResponse::from(&session));
    }

    let session = Session::new(name, auth.openid, short_id, backend);

    let sessions = kv.open_table("sessions").await.map_db_err()?;
    let sessions_by_objid = kv.open_table("sessions_by_objid").await.map_db_err()?;

    kv_put(&sessions, &session.session_token, &session).await?;
    kv_put(&sessions_by_openid, &session.openid, &session.session_token).await?;
    kv_put(
        &sessions_by_objid,
        &session.object_id,
        &session.session_token,
    )
    .await?;
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
) -> Result<(), PCSError> {
    let mut session = get_session_by_object_id(backend, object_id).await?;
    session.nickname = params.nickname;
    session.updated_at = backend.get_utc_now();
    save_session(backend, &session).await?;
    backend
        .emit_event(Event::UserUpdate {
            user: EventUser::from(&session),
        })
        .await;
    Ok(())
}

pub async fn handle_delete<B: PCSBackend>(
    backend: &B,
    object_id: &str,
    session_token: &str,
) -> Result<(), PCSError> {
    let session = get_session_by_token(backend, session_token).await?;
    if session.object_id != object_id {
        return Err(PCSError::unauthorized("not authorized to delete this user"));
    }
    let kv = backend.kv().await;
    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;
    let prefix = format!("{}:", session.object_id);
    let keys = games_by_user.list_keys(&prefix).await.map_internal_err()?;
    for key in &keys {
        if let Some(gs_objid) = key.strip_prefix(&prefix) {
            if let Some(gs) = kv_get::<GameSave, _>(&game_saves, gs_objid).await? {
                let _ = file::handle_delete(backend, &gs.game_file_object_id).await;
            }
            kv_delete(&game_saves, gs_objid).await?;
            kv_delete(&games_by_user, key).await?;
        }
    }

    delete_session_tables(backend, &session).await?;
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
            "not authorized to refresh this session",
        ));
    }

    let old_event_user = EventUser::from(&session);

    let kv = backend.kv().await;
    let sessions = kv.open_table("sessions").await.map_db_err()?;
    kv_delete(&sessions, &session.session_token).await?;

    session.session_token = backend.random_id();
    session.updated_at = backend.get_utc_now();
    kv_put(&sessions, &session.session_token, &session).await?;

    let sessions_by_objid = kv.open_table("sessions_by_objid").await.map_db_err()?;
    sessions_by_objid
        .put(&session.object_id, session.session_token.as_bytes())
        .await
        .map_internal_err()?;

    backend
        .emit_event(Event::UserRefreshSessionToken {
            user: old_event_user,
        })
        .await;
    Ok(SessionResponse::from(&session))
}
