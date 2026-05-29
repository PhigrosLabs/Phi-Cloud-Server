use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use base64::Engine;
use chrono::{DateTime, SecondsFormat, Utc};
pub mod http;
use futures::TryStreamExt;
pub(crate) use http::*;
pub mod error;
pub(crate) use error::*;

use crate::types::{ByteStream, error::PCSError};

pub fn decode_base64_key(encoded: &str) -> Result<String, PCSError> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .map_err(|e| PCSError::bad_request(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| PCSError::bad_request(e.to_string()))
}

pub trait ToRfc3339Z {
    fn to_rfc3339_z(&self) -> String;
}

impl ToRfc3339Z for DateTime<Utc> {
    fn to_rfc3339_z(&self) -> String {
        self.to_rfc3339_opts(SecondsFormat::Secs, true)
    }
}

pub async fn stream_to_bytes<S: ByteStream>(stream: S) -> Result<Vec<u8>, S::Error> {
    let chunks: Vec<Vec<u8>> = stream.try_collect().await?;
    let total_len: usize = chunks.iter().map(|c| c.len()).sum();
    let mut result = Vec::with_capacity(total_len);
    for chunk in chunks {
        result.extend_from_slice(&chunk);
    }

    Ok(result)
}
