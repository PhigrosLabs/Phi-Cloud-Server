use async_trait::async_trait;
use redb::{Database, ReadableDatabase, TableDefinition};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;

use pcs_core::types::kv::{KVStorage, KVTable};

#[derive(Clone)]
pub struct RedbKVStorage {
    db: Arc<Database>,
}

impl RedbKVStorage {
    pub fn new(path: &str) -> Result<Self, redb::Error> {
        let db = Database::create(path)?;
        Ok(Self { db: Arc::new(db) })
    }
}

#[derive(Clone)]
pub struct RedbKVTable {
    db: Arc<Database>,
    table_name: String,
}

#[async_trait]
impl KVStorage for RedbKVStorage {
    type Table = RedbKVTable;
    type Error = redb::Error;

    async fn open_table(&self, table: &str) -> Result<Self::Table, Self::Error> {
        Ok(RedbKVTable {
            db: self.db.clone(),
            table_name: table.to_string(),
        })
    }
}

#[async_trait]
impl KVTable for RedbKVTable {
    type Error = redb::Error;

    async fn get<T>(&self, key: &str) -> Result<Option<T>, Self::Error>
    where
        T: DeserializeOwned + Send + Sync,
    {
        let tab_def: TableDefinition<&str, Vec<u8>> = TableDefinition::new(&self.table_name);

        let txn = self.db.begin_read()?;
        let table = match txn.open_table(tab_def) {
            Ok(table) => table,
            Err(redb::TableError::TableDoesNotExist(_)) => {
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };

        let value = table.get(key)?;

        let result = match value {
            Some(v) => {
                let bytes = v.value();
                let decoded = serde_json::from_slice::<T>(&bytes).map_err(|e| {
                    redb::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                })?;
                Some(decoded)
            }
            None => None,
        };

        Ok(result)
    }

    async fn put<T>(&self, key: &str, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize + Send + Sync,
    {
        let value = serde_json::to_vec(value).map_err(|e| {
            redb::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;

        let tab_def: TableDefinition<&str, Vec<u8>> = TableDefinition::new(&self.table_name);
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(tab_def)?;
            table.insert(key, value.to_vec())?;
        }
        txn.commit()?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), Self::Error> {
        let tab_def: TableDefinition<&str, Vec<u8>> = TableDefinition::new(&self.table_name);
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(tab_def)?;
            table.remove(key)?;
        }
        txn.commit()?;
        Ok(())
    }
}
