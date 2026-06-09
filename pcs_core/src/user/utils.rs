use alloc::string::String;

use crate::{
    types::{
        backend::PCSBackend,
        error::{ErrorCode, PCSError},
        kv::KVStorage,
    },
    user::model::{Session, SessionTokenByObjId, SessionTokenByOpenId},
    utils::*,
};

pub async fn get_session_by_token<B: PCSBackend>(
    backend: &B,
    token: &str,
) -> Result<Session, PCSError> {
    let kv = backend.kv();
    kv.get::<Session>(token)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .ok_or_else(PCSError::db_not_found)
}

pub async fn get_session_by_object_id<B: PCSBackend>(
    backend: &B,
    object_id: &str,
) -> Result<Session, PCSError> {
    let kv = backend.kv();
    let SessionTokenByObjId(token) = kv
        .get::<SessionTokenByObjId>(object_id)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .ok_or_else(PCSError::db_not_found)?;
    get_session_by_token(backend, &token).await
}

pub async fn get_session_token_by_openid<B: PCSBackend>(
    backend: &B,
    openid: &str,
) -> Result<Option<String>, PCSError> {
    let kv = backend.kv();
    Ok(kv
        .get::<SessionTokenByOpenId>(openid)
        .await
        .map_pcs_error(ErrorCode::KV_GET)?
        .map(|SessionTokenByOpenId(token)| token))
}

pub async fn save_session<B: PCSBackend>(backend: &B, session: &Session) -> Result<(), PCSError> {
    let kv = backend.kv();
    kv.put::<Session>(&session.session_token, session)
        .await
        .map_pcs_error(ErrorCode::KV_PUT)
}

pub async fn put_session_with_indices<B: PCSBackend>(
    backend: &B,
    session: &Session,
) -> Result<(), PCSError> {
    let kv = backend.kv();
    kv.put::<Session>(&session.session_token, session)
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
    Ok(())
}

// ── Session 删除 ──

pub async fn delete_session<B: PCSBackend>(backend: &B, session: &Session) -> Result<(), PCSError> {
    let kv = backend.kv();
    kv.delete::<Session>(&session.session_token)
        .await
        .map_pcs_error(ErrorCode::KV_DELETE)?;
    kv.delete::<SessionTokenByOpenId>(&session.openid)
        .await
        .map_pcs_error(ErrorCode::KV_DELETE)?;
    kv.delete::<SessionTokenByObjId>(&session.object_id)
        .await
        .map_pcs_error(ErrorCode::KV_DELETE)?;
    Ok(())
}
