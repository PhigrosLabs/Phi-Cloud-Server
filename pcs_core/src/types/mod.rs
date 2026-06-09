use core::error::Error;

use alloc::{string::String, vec, vec::Vec};

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
use serde::{Deserialize, Serialize};

use crate::user::model::Session;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ACL(
    #[serde(with = "tuple_vec_map")] pub Vec<(String, Permission)>, // e.g. { "user_obj_id": { read: true, write: true } }
);

impl ACL {
    pub fn empty() -> Self {
        ACL(Vec::new())
    }

    pub fn from_user(session: &Session) -> Self {
        ACL(vec![(
            session.object_id.clone(),
            Permission {
                read: true,
                write: true,
            },
        )])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub read: bool,
    pub write: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Date {
    #[serde(rename = "__type")]
    type_field: String,
    pub iso: String,
}

impl Date {
    pub fn new(iso: impl Into<String>) -> Self {
        Self {
            type_field: "Date".into(),
            iso: iso.into(),
        }
    }
}
pub const SVG_CONTENT_TYPE: &str = "image/svg+xml; charset=utf-8";
pub const JSON_CONTENT_TYPE: &str = "application/json; charset=utf-8";
pub const OCTET_STREAM_CONTENT_TYPE: &str = "application/octet-stream";
