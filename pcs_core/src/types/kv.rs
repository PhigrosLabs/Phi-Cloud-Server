use core::error::Error;

use trait_variant::make;

#[make(Send)]
pub trait KVStorage: Send + Sync + 'static {
    type Table: KVTable;
    type Error: Error;

    async fn open_table(&self, table: &str) -> Result<Self::Table, Self::Error>;
}

#[make(Send)]
pub trait KVTable: Send + Sync {
    type Error: Error;

    async fn get<T>(&self, key: &str) -> Result<Option<T>, Self::Error>
    where
        T: serde::de::DeserializeOwned + Send + Sync;

    async fn put<T>(&self, key: &str, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize + Send + Sync;

    async fn delete(&self, key: &str) -> Result<(), Self::Error>;
}
