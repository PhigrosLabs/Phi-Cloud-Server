use core::error::Error;

use alloc::{collections::btree_map::BTreeMap, string::String, vec::Vec};

pub mod backend;
pub mod error;
pub mod event;
pub mod file_bucket;
pub mod http;
pub mod kv;
pub use backend::*;
pub use error::*;
pub use file_bucket::*;
use futures::Stream;
pub use http::*;
pub use kv::*;

pub type ACL = BTreeMap<String, BTreeMap<String, bool>>;

pub trait ByteStream: Stream<Item = Result<Vec<u8>, Self::Error>> + Send + 'static {
    type Error: Error;
}

impl<T, E> ByteStream for T
where
    T: Stream<Item = Result<Vec<u8>, E>> + Send + 'static,
    E: Error,
{
    type Error = E;
}

pub const JSON_CONTENT_TYPE: &str = "application/json; charset=utf-8";
pub const OCTET_STREAM_CONTENT_TYPE: &str = "application/octet-stream";
