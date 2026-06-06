use std::sync::Arc;

use pcs_core::types::kv::{KVStorage, KVTable};
use redb::{Database, ReadableDatabase, TableDefinition};

pub struct RedbKVStorage {
    db: Arc<Database>,
}

impl RedbKVStorage {
    pub fn new(path: &str) -> Result<Self, redb::Error> {
        let db = Database::create(path)?;
        Ok(Self { db: Arc::new(db) })
    }
}

type Table = TableDefinition<'static, &'static str, Vec<u8>>;

impl KVStorage for RedbKVStorage {
    type Error = redb::Error;

    async fn get<T: KVTable>(&self, key: &str) -> Result<Option<T>, Self::Error> {
        let db = self.db.clone();
        let key = key.to_owned();
        tokio::task::spawn_blocking(move || {
            let table: Table = TableDefinition::new(T::TABLE_NAME);
            let txn = db.begin_read()?;
            let table = match txn.open_table(table) {
                Ok(table) => table,
                Err(redb::TableError::TableDoesNotExist(_)) => {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
            };

            let value = table.get(key.as_str())?;
            match value {
                Some(v) => {
                    let bytes = v.value();
                    let decoded = serde_json::from_slice::<T>(&bytes).map_err(|e| {
                        redb::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                    })?;
                    Ok(Some(decoded))
                }
                None => Ok(None),
            }
        })
        .await
        .expect("spawn_blocking panicked")
    }

    async fn put<T: KVTable>(&self, key: &str, value: &T) -> Result<(), Self::Error> {
        let db = self.db.clone();
        let key = key.to_owned();
        let value_bytes = serde_json::to_vec(value).map_err(|e| {
            redb::Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;

        tokio::task::spawn_blocking(move || {
            let table: Table = TableDefinition::new(T::TABLE_NAME);
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(table)?;
                table.insert(key.as_str(), value_bytes)?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .expect("spawn_blocking panicked")
    }

    async fn delete<T: KVTable>(&self, key: &str) -> Result<(), Self::Error> {
        let db = self.db.clone();
        let key = key.to_owned();

        tokio::task::spawn_blocking(move || {
            let table: Table = TableDefinition::new(T::TABLE_NAME);
            let txn = db.begin_write()?;
            {
                let mut table = txn.open_table(table)?;
                table.remove(key.as_str())?;
            }
            txn.commit()?;
            Ok(())
        })
        .await
        .expect("spawn_blocking panicked")
    }
}
