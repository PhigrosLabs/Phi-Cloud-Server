use pcs_core::types::kv::{KVStorage, KVTable};
use worker::*;

use crate::utils::UnsafeSend;

#[derive(Clone)]
pub struct WorkerKVStorage {
    pub kv: KvStore,
}

impl KVStorage for WorkerKVStorage {
    type Error = worker::Error;

    async fn get<T: KVTable>(&self, key: &str) -> Result<Option<T>, Self::Error> {
        let pk = format!("{}:{}", T::TABLE_NAME, key);

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

    async fn put<T: KVTable>(&self, key: &str, value: &T) -> Result<(), Self::Error> {
        let pk = format!("{}:{}", T::TABLE_NAME, key);
        let kv = self.kv.clone();
        let bytes =
            serde_json::to_vec(value).map_err(|e| worker::Error::RustError(e.to_string()))?;

        UnsafeSend(async move {
            kv.put_bytes(&pk, &bytes)?.execute().await?;
            Ok(())
        })
        .await
    }

    async fn delete<T: KVTable>(&self, key: &str) -> Result<(), Self::Error> {
        let pk = format!("{}:{}", T::TABLE_NAME, key);
        let kv = self.kv.clone();

        UnsafeSend(async move {
            kv.delete(&pk).await?;
            Ok(())
        })
        .await
    }
}
