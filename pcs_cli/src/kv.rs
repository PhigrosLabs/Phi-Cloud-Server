use async_trait::async_trait;
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
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

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Self::Error> {
        let tab_def: TableDefinition<&str, Vec<u8>> = TableDefinition::new(&self.table_name);
        let txn = self.db.begin_read()?;
        let table = txn.open_table(tab_def)?;
        let value = table.get(key)?;
        Ok(value.map(|v: redb::AccessGuard<Vec<u8>>| v.value().clone()))
    }

    async fn put(&self, key: &str, value: &[u8]) -> Result<(), Self::Error> {
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

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, Self::Error> {
        let tab_def: TableDefinition<&str, Vec<u8>> = TableDefinition::new(&self.table_name);
        let txn = self.db.begin_read()?;
        let table = txn.open_table(tab_def)?;

        let mut results = Vec::new();
        for item in table.iter()? {
            let (k, _): (_, redb::AccessGuard<Vec<u8>>) = item?;
            let key_str = k.value().to_string();
            if key_str.starts_with(prefix) {
                results.push(key_str);
            }
        }
        Ok(results)
    }
}
