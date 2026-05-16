use async_trait::async_trait;
use pcs_core::types::kv::{KVStorage, KVTable};
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

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        let pk = self.prefixed_key(key);
        UnsafeSend(async move { Ok(self.kv.get(&pk).bytes().await?) }).await
    }

    async fn put(&self, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        let pk = self.prefixed_key(key);
        UnsafeSend(async move {
            self.kv.put_bytes(&pk, &value)?.execute().await?;
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

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Self::Error> {
        let full_prefix = self.prefixed_key(prefix);
        let prefix_len = self.table_prefix.len() + 1; // "table:"
        UnsafeSend(async move {
            let list = self.kv.list().prefix(full_prefix).execute().await?;
            let keys: Vec<String> = list
                .keys
                .into_iter()
                .map(|k| k.name[prefix_len..].to_string())
                .collect();
            Ok(keys)
        })
        .await
    }
}
