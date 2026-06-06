use alloc::{string::ToString, vec::Vec};

use crate::{
    file,
    game::model::{GameSave, GameSaveIdsByUser},
    types::{
        backend::PCSBackend,
        error::{ErrorCode, PCSError},
        event::{Event, EventUser},
        kv::KVStorage,
    },
    user::{
        AuthData, SessionResponse, UpdateUserParams, UserResponse, delete_session_tables,
        get_session_by_object_id, get_session_by_token,
        model::{Session, SessionTokenByObjId, SessionTokenByOpenId},
        save_session,
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

    if let Some(SessionTokenByOpenId(token)) = kv
        .get::<SessionTokenByOpenId>(&auth.openid)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
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

    kv.put::<Session>(&session.session_token, &session)
        .await
        .map_pcs_error(ErrorCode::KV_PUT)?;
    kv.put::<SessionTokenByOpenId>(
        &session.openid,
        &SessionTokenByOpenId(session.session_token.clone()),
    )
    .await
    .map_pcs_error(ErrorCode::KV_PUT)?;
    kv.put::<SessionTokenByObjId>(
        &session.object_id,
        &SessionTokenByObjId(session.session_token.clone()),
    )
    .await
    .map_pcs_error(ErrorCode::KV_PUT)?;
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
    let kv = backend.kv();
    let GameSaveIdsByUser(gs_ids) = kv
        .get::<GameSaveIdsByUser>(&session.object_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .unwrap_or(GameSaveIdsByUser(Vec::new()));
    for gs_objid in &gs_ids {
        if let Some(gs) = kv
            .get::<GameSave>(gs_objid)
            .await
            .map_pcs_error(ErrorCode::KV_GET)?
        {
            file::handle_delete(backend, &gs.game_file_object_id).await?;
        }
        kv.delete::<GameSave>(gs_objid)
            .await
            .map_pcs_error(ErrorCode::KV_DELETE)?;
    }
    kv.delete::<GameSaveIdsByUser>(&session.object_id)
        .await
        .map_pcs_error(ErrorCode::KV_DELETE)?;

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
            ErrorCode::UNAUTHORIZED_REFRESH_SESSION,
            "not authorized to refresh this session",
        ));
    }

    let old_event_user = EventUser::from(&session);

    let kv = backend.kv();
    kv.delete::<Session>(&session.session_token)
        .await
        .map_pcs_error(ErrorCode::KV_DELETE)?;

    session.session_token = backend.random_id();
    session.updated_at = backend.utc_now();
    kv.put::<Session>(&session.session_token, &session)
        .await
        .map_pcs_error(ErrorCode::KV_PUT)?;
    kv.put::<SessionTokenByObjId>(
        &session.object_id,
        &SessionTokenByObjId(session.session_token.clone()),
    )
    .await
    .map_pcs_error(ErrorCode::KV_PUT)?;
    kv.put::<SessionTokenByOpenId>(
        &session.openid,
        &SessionTokenByOpenId(session.session_token.clone()),
    )
    .await
    .map_pcs_error(ErrorCode::KV_PUT)?;

    backend
        .emit_event(Event::UserRefreshSessionToken {
            user: old_event_user,
            new: EventUser::from(&session),
        })
        .await;
    Ok(SessionResponse::from(&session))
}
