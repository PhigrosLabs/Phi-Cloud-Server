use alloc::string::String;

use crate::{
    types::{
        backend::PCSBackend,
        error::PCSError,
        kv::{KVStorage, KVTable},
    },
    user::model::Session,
    utils::*,
};

pub async fn get_session_by_token<B: PCSBackend>(
    backend: &B,
    token: &str,
) -> Result<Session, PCSError> {
    let kv = backend.kv();
    let sessions = kv.open_table("sessions").await.map_db_err()?;
    return sessions
        .get::<Session>(token)
        .await
        .map_db_err()?
        .ok_or_else(PCSError::db_not_found);
}

pub async fn get_session_by_object_id<B: PCSBackend>(
    backend: &B,
    object_id: &str,
) -> Result<Session, PCSError> {
    let kv = backend.kv();
    let sessions_by_objid = kv.open_table("sessions_by_objid").await.map_db_err()?;
    let token: String = sessions_by_objid
        .get(object_id)
        .await
        .map_db_err()?
        .ok_or_else(PCSError::db_not_found)?;
    get_session_by_token(backend, &token).await
}

pub async fn save_session<B: PCSBackend>(backend: &B, session: &Session) -> Result<(), PCSError> {
    let kv = backend.kv();
    let sessions = kv.open_table("sessions").await.map_db_err()?;
    sessions
        .put(&session.session_token, session)
        .await
        .map_db_err()
}

pub async fn delete_session_tables<B: PCSBackend>(
    backend: &B,
    session: &Session,
) -> Result<(), PCSError> {
    let kv = backend.kv();
    let sessions = kv.open_table("sessions").await.map_db_err()?;
    let sessions_by_openid = kv.open_table("sessions_by_openid").await.map_db_err()?;
    let sessions_by_objid = kv.open_table("sessions_by_objid").await.map_db_err()?;
    sessions.delete(&session.session_token).await.map_db_err()?;
    sessions_by_openid
        .delete(&session.openid)
        .await
        .map_db_err()?;
    sessions_by_objid
        .delete(&session.object_id)
        .await
        .map_db_err()?;
    Ok(())
}
