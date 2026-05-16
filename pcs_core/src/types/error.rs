use http::{Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum ErrorCode {
    DbError,
    DbNotFound,
    Other(u32),
}

#[derive(Debug, Clone, Serialize)]
pub struct PCSError {
    #[serde(rename = "code")]
    pub http_code: u16,
    #[serde(rename = "internal_code")]
    pub tcs_code: Option<ErrorCode>,
    #[serde(rename = "error")]
    pub message: String,
}

impl PCSError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            http_code: 400,
            tcs_code: None,
            message: msg.into(),
        }
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self {
            http_code: 401,
            tcs_code: None,
            message: msg.into(),
        }
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            http_code: 403,
            tcs_code: None,
            message: msg.into(),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            http_code: 404,
            tcs_code: None,
            message: msg.into(),
        }
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            http_code: 500,
            tcs_code: None,
            message: msg.into(),
        }
    }

    pub fn db_not_found() -> Self {
        Self {
            http_code: 500,
            tcs_code: Some(ErrorCode::DbNotFound),
            message: "data not found".into(),
        }
    }

    pub fn db_error(msg: impl Into<String>) -> Self {
        Self {
            http_code: 500,
            tcs_code: Some(ErrorCode::DbError),
            message: msg.into(),
        }
    }
}

impl fmt::Display for PCSError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PCSError {}

impl From<PCSError> for Response<Vec<u8>> {
    fn from(err: PCSError) -> Self {
        Response::builder()
            .status(
                StatusCode::from_u16(err.http_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            )
            .header("Content-Type", "application/json; charset=utf-8")
            .body(serde_json::to_vec(&err).unwrap_or_default())
            .unwrap()
    }
}
