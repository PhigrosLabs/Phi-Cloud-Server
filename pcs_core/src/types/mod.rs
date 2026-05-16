use std::collections::HashMap;

pub mod backend;
pub mod error;
pub mod event;
pub mod file_bucket;
pub mod kv;

pub type ACL = HashMap<String, HashMap<String, bool>>;
