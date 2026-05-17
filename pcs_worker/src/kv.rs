use async_trait::async_trait;
use pcs_core::types::kv::{KVStorage, KVTable};
use serde::{Serialize, de::DeserializeOwned};
use worker::*;

use crate::utils::UnsafeSend;

#[derive(Clone)]
pub struct WorkerKVStorage {
    pub kv: KvStore,
    pub table_prefix: String,
}

impl WorkerKVStorage {
    fn prefixed_key(&self, key: &str) -> String {
        format!("{}:{}", self.table_prefix, key)
    }
}

#[async_trait]
impl KVStorage for WorkerKVStorage {
    type Table = Self;
    type Error = worker::Error;

    async fn open_table(&self, table: &str) -> Result<Self::Table, Self::Error> {
        Ok(Self {
            kv: self.kv.clone(),
            table_prefix: table.to_string(),
        })
    }
}

#[async_trait]
impl KVTable for WorkerKVStorage {
    type Error = worker::Error;

    async fn get<T>(&self, key: &str) -> Result<Option<T>, Self::Error>
    where
        T: DeserializeOwned + Send + Sync,
    {
        let pk = self.prefixed_key(key);

        UnsafeSend(async move {
            let opt = self.kv.get(&pk).bytes().await?;

            match opt {
                Some(bytes) => {
                    let v = serde_json::from_slice::<T>(&bytes)
                        .map_err(|e| worker::Error::RustError(e.to_string()))?;
                    Ok(Some(v))
                }
                None => Ok(None),
            }
        })
        .await
    }

    async fn put<T>(&self, key: &str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + Send + Sync,
    {
        let pk = self.prefixed_key(key);

        UnsafeSend(async move {
            let bytes =
                serde_json::to_vec(value).map_err(|e| worker::Error::RustError(e.to_string()))?;

            self.kv.put_bytes(&pk, &bytes)?.execute().await?;
            Ok(())
        })
        .await
    }

    async fn delete(&self, key: &str) -> Result<(), Self::Error> {
        let pk = self.prefixed_key(key);

        UnsafeSend(async move {
            self.kv.delete(&pk).await?;
            Ok(())
        })
        .await
    }
}
