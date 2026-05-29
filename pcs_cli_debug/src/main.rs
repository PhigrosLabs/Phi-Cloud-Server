use clap::{Parser, Subcommand};
use redb::{
    Database, ReadTransaction, ReadableDatabase, ReadableTable, TableDefinition, TableHandle,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pcs_cli_debug", about = "Phi Cloud Server Debug Tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Dump all tables from a redb database to ./kv.json
    DumpDb {
        /// Path to the redb database
        #[arg(default_value = "./data/kv.db")]
        db_path: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Command::DumpDb { db_path } => {
            dump_db(db_path.to_str().expect("invalid db path"))?;
        }
    }

    Ok(())
}

fn dump_db(db_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let db = Database::open(db_path)?;
    let txn = db.begin_read()?;

    let mut all_data: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();

    let tables = list_tables(&txn)?;

    for table_name in &tables {
        let tab_def: TableDefinition<&str, Vec<u8>> = TableDefinition::new(table_name);

        let table = match txn.open_table(tab_def) {
            Ok(t) => t,
            Err(_) => {
                eprintln!(
                    "Warning: skipping table '{}' (incompatible type)",
                    table_name
                );
                continue;
            }
        };

        let mut table_data = BTreeMap::new();

        for entry in table.iter()? {
            let (key, value): (redb::AccessGuard<&str>, redb::AccessGuard<Vec<u8>>) = entry?;
            let key_str = key.value().to_string();
            let value_bytes = value.value();

            let json_value = match serde_json::from_slice::<Value>(&value_bytes) {
                Ok(v) => v,
                Err(_) => Value::String(format!(
                    "<non-json binary data, {} bytes>",
                    value_bytes.len()
                )),
            };

            table_data.insert(key_str, json_value);
        }

        all_data.insert(table_name.clone(), table_data);
    }

    let json = serde_json::to_string_pretty(&all_data)?;
    std::fs::write("./.kv.json", json)?;

    println!(
        "Dumped {} table(s) with {} total entries to ./.kv.json",
        all_data.len(),
        all_data.values().map(|t| t.len()).sum::<usize>()
    );

    Ok(())
}

fn list_tables(txn: &ReadTransaction) -> Result<Vec<String>, redb::Error> {
    let mut tables = Vec::new();

    let list = txn.list_tables()?;

    for handle in list {
        tables.push(handle.name().to_string());
    }

    Ok(tables)
}
