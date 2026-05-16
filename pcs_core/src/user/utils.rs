use crate::{
    types::{backend::PCSBackend, error::PCSError, kv::KVStorage},
    user::model::Session,
    utils::*,
};

pub async fn get_session_by_token<B: PCSBackend>(
    backend: &B,
    token: &str,
) -> Result<Session, PCSError> {
    let kv = backend.kv().await;
    let sessions = kv.open_table("sessions").await.map_db_err()?;
    kv_get(&sessions, token)
        .await?
        .ok_or_else(PCSError::db_not_found)
}

pub async fn get_session_by_object_id<B: PCSBackend>(
    backend: &B,
    object_id: &str,
) -> Result<Session, PCSError> {
    let kv = backend.kv().await;
    let sessions_by_objid = kv.open_table("sessions_by_objid").await.map_db_err()?;
    let token: String = kv_get(&sessions_by_objid, object_id)
        .await?
        .ok_or_else(PCSError::db_not_found)?;
    get_session_by_token(backend, &token).await
}

pub async fn save_session<B: PCSBackend>(backend: &B, session: &Session) -> Result<(), PCSError> {
    let kv = backend.kv().await;
    let sessions = kv.open_table("sessions").await.map_db_err()?;
    kv_put(&sessions, &session.session_token, session).await
}

pub async fn delete_session_tables<B: PCSBackend>(
    backend: &B,
    session: &Session,
) -> Result<(), PCSError> {
    let kv = backend.kv().await;
    let sessions = kv.open_table("sessions").await.map_db_err()?;
    let sessions_by_openid = kv.open_table("sessions_by_openid").await.map_db_err()?;
    let sessions_by_objid = kv.open_table("sessions_by_objid").await.map_db_err()?;
    kv_delete(&sessions, &session.session_token).await?;
    kv_delete(&sessions_by_openid, &session.openid).await?;
    kv_delete(&sessions_by_objid, &session.object_id).await?;
    Ok(())
}
