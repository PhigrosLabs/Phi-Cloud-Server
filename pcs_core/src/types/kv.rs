use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait KVStorage: Send + Sync + 'static {
    type Table: KVTable;
    type Error: Error + Send;

    async fn open_table(&self, table: &str) -> Result<Self::Table, Self::Error>;
}

#[async_trait]
pub trait KVTable: Send + Sync {
    type Error: Error + Send;

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<(), Self::Error>;
    async fn delete(&self, key: &str) -> Result<(), Self::Error>;

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Self::Error>;
}
