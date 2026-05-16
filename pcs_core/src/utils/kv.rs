use serde::{Serialize, de::DeserializeOwned};

use crate::{
    types::{error::PCSError, kv::KVTable},
    utils::MapPCSError,
};

pub(crate) async fn kv_get<T: DeserializeOwned, K: KVTable>(
    table: &K,
    key: &str,
) -> Result<Option<T>, PCSError> {
    let Some(data) = table.get(key).await.map_bad_err()? else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_slice(&data).map_internal_err()?))
}

pub(crate) async fn kv_put<T: Serialize, K: KVTable>(
    table: &K,
    key: &str,
    value: &T,
) -> Result<(), PCSError> {
    table
        .put(key, &serde_json::to_vec(value).map_internal_err()?)
        .await
        .map_internal_err()
}

pub(crate) async fn kv_delete<K: KVTable>(table: &K, key: &str) -> Result<(), PCSError> {
    table.delete(key).await.map_internal_err()
}
