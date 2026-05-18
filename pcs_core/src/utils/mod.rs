use alloc::string::{String, ToString};
use base64::Engine;
use chrono::{DateTime, SecondsFormat, Utc};
pub(crate) mod http;
pub(crate) use http::*;
pub(crate) mod error;
pub(crate) use error::*;

use crate::types::error::PCSError;

pub(crate) fn decode_base64_key(encoded: &str) -> Result<String, PCSError> {
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
