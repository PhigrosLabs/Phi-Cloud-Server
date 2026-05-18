use alloc::{
    string::{String, ToString},
    vec::Vec,
};

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

use crate::utils::MapPCSError;

pub async fn handle_register<B: PCSBackend>(
    backend: &B,
    auth: AuthData,
) -> Result<SessionResponse, PCSError> {
    let check_result = backend.user_check(&auth).await?;
    let name = check_result.name.unwrap_or(auth.name);
    let short_id = check_result.short_id.unwrap_or_else(|| "PCS".to_string());

    let kv = backend.kv();
    let sessions_by_openid = kv.open_table("sessions_by_openid").await.map_db_err()?;

    if let Some(token) = sessions_by_openid
        .get::<String>(&auth.openid)
        .await
        .map_db_err()?
    {
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

    sessions
        .put(&session.session_token, &session)
        .await
        .map_db_err()?;
    sessions_by_openid
        .put(&session.openid, &session.session_token)
        .await
        .map_db_err()?;
    sessions_by_objid
        .put(&session.object_id, &session.session_token)
        .await
        .map_db_err()?;
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
    session.updated_at = backend.get_utc_now();
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
        return Err(PCSError::unauthorized("not authorized to delete this user"));
    }
    let kv = backend.kv();
    let games_by_user = kv.open_table("game_saves_by_user").await.map_db_err()?;
    let game_saves = kv.open_table("game_saves").await.map_db_err()?;
    let gs_ids: Vec<String> = games_by_user
        .get(&session.object_id)
        .await
        .map_db_err()?
        .unwrap_or_default();
    for gs_objid in &gs_ids {
        if let Some(gs) = game_saves.get::<GameSave>(gs_objid).await.map_db_err()? {
            file::handle_delete(backend, &gs.game_file_object_id).await?;
        }
        game_saves.delete(gs_objid).await.map_db_err()?;
    }
    games_by_user
        .delete(&session.object_id)
        .await
        .map_db_err()?;

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

    let kv = backend.kv();
    let sessions = kv.open_table("sessions").await.map_db_err()?;
    sessions.delete(&session.session_token).await.map_db_err()?;

    session.session_token = backend.random_id();
    session.updated_at = backend.get_utc_now();
    sessions
        .put(&session.session_token, &session)
        .await
        .map_db_err()?;
    let sessions_by_objid = kv.open_table("sessions_by_objid").await.map_db_err()?;
    sessions_by_objid
        .put(&session.object_id, &session.session_token)
        .await
        .map_internal_err()?;
    let sessions_by_openid = kv.open_table("sessions_by_openid").await.map_db_err()?;
    sessions_by_openid
        .put(&session.openid, &session.session_token)
        .await
        .map_internal_err()?;

    backend
        .emit_event(Event::UserRefreshSessionToken {
            user: old_event_user,
            new: EventUser::from(&session),
        })
        .await;
    Ok(SessionResponse::from(&session))
}
