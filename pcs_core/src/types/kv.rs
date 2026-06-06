use core::error::Error;

use serde::{Serialize, de::DeserializeOwned};
use trait_variant::make;

#[make(Send)]
pub trait KVStorage: Send + Sync + 'static {
    type Error: Error;

    async fn get<T: KVTable>(&self, key: &str) -> Result<Option<T>, Self::Error>;
    async fn put<T: KVTable>(&self, key: &str, value: &T) -> Result<(), Self::Error>;
    async fn delete<T: KVTable>(&self, key: &str) -> Result<(), Self::Error>;
}

pub trait KVTable: DeserializeOwned + Send + Sync + Serialize + 'static {
    const TABLE_NAME: &'static str;
}
