use alloc::{boxed::Box, collections::btree_map::BTreeMap, string::String};
use bytes::Bytes;
use futures::Stream;

pub mod backend;
pub mod error;
pub mod event;
pub mod file_bucket;
pub mod kv;

pub type ACL = BTreeMap<String, BTreeMap<String, bool>>;

pub type PcsBody = Option<Box<dyn Stream<Item = Bytes> + Send + Sync + Unpin + 'static>>;
