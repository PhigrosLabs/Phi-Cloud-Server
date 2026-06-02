use alloc::{fmt, string::String};
use serde::{Deserialize, Serialize};

use crate::types::{Body, ByteStream, JSON_CONTENT_TYPE, Response};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ErrorCode(pub u32);

impl ErrorCode {
    pub const DB_ERROR: ErrorCode = ErrorCode(11);
    pub const DB_NOT_FOUND: ErrorCode = ErrorCode(12);

    pub const KV_OPEN_TABLE: ErrorCode = ErrorCode(20);
    pub const KV_GET: ErrorCode = ErrorCode(21);
    pub const KV_PUT: ErrorCode = ErrorCode(22);
    pub const KV_DELETE: ErrorCode = ErrorCode(23);

    pub const FB_HEAD: ErrorCode = ErrorCode(30);
    pub const FB_GET: ErrorCode = ErrorCode(31);
    pub const FB_DELETE: ErrorCode = ErrorCode(32);
    pub const FB_CREATE_MULTIPART_UPLOAD: ErrorCode = ErrorCode(33);
    pub const FB_GET_MULTIPART_UPLOAD: ErrorCode = ErrorCode(34);
    pub const FB_UPLOAD_PART: ErrorCode = ErrorCode(35);
    pub const FB_MULTIPART_COMPLETE: ErrorCode = ErrorCode(36);
    pub const FB_PUT: ErrorCode = ErrorCode(37);

    pub const JSON_SERIALIZE: ErrorCode = ErrorCode(40);
    pub const JSON_DESERIALIZE: ErrorCode = ErrorCode(41);
    pub const BASE64_DECODE: ErrorCode = ErrorCode(42);
    pub const UTF8_CONVERT: ErrorCode = ErrorCode(43);
    pub const SAVE_DATA_PARSE: ErrorCode = ErrorCode(44);
    pub const TEMPLATE_RENDER: ErrorCode = ErrorCode(45);

    pub const PHI_INFO_VERSION_MISMATCH: ErrorCode = ErrorCode(50);

    pub const INVALID_PART_NUMBER: ErrorCode = ErrorCode(61);
    pub const ROUTE_NOT_FOUND: ErrorCode = ErrorCode(62);
    pub const MISSING_SESSION_TOKEN: ErrorCode = ErrorCode(63);

    pub const UNAUTHORIZED_DELETE_USER: ErrorCode = ErrorCode(71);
    pub const UNAUTHORIZED_REFRESH_SESSION: ErrorCode = ErrorCode(72);

    pub const B30_NO_GAME_SAVES_FOUND: ErrorCode = ErrorCode(81);
    pub const B30_INVALID_SAVE_DATA: ErrorCode = ErrorCode(82);
    pub const B30_GET_GAME_RECORD: ErrorCode = ErrorCode(83);
    pub const B30_GET_USER: ErrorCode = ErrorCode(84);
    pub const B30_GET_GAME_PROGRESS: ErrorCode = ErrorCode(85);
    pub const B30_GET_SETTINGS: ErrorCode = ErrorCode(86);

    pub const SAVE_NO_GAME_SAVES_FOUND: ErrorCode = ErrorCode(91);
    pub const SAVE_INVALID_SAVE_DATA: ErrorCode = ErrorCode(92);
    pub const SAVE_GET_GAME_KEY: ErrorCode = ErrorCode(93);
    pub const SAVE_GET_GAME_RECORD: ErrorCode = ErrorCode(94);
    pub const SAVE_GET_GAME_PROGRESS: ErrorCode = ErrorCode(95);
    pub const SAVE_GET_SETTINGS: ErrorCode = ErrorCode(96);
    pub const SAVE_GET_USER: ErrorCode = ErrorCode(97);
    pub const SAVE_SET_GAME_KEY: ErrorCode = ErrorCode(98);
    pub const SAVE_SET_GAME_RECORD: ErrorCode = ErrorCode(99);
    pub const SAVE_SET_GAME_PROGRESS: ErrorCode = ErrorCode(901);
    pub const SAVE_SET_SETTINGS: ErrorCode = ErrorCode(902);
    pub const SAVE_SET_USER: ErrorCode = ErrorCode(903);
    pub const SAVE_BUILD: ErrorCode = ErrorCode(904);
    pub const SAVE_NO_GAME_SAVES_FOUND2: ErrorCode = ErrorCode(905);
    pub const SAVE_INVALID_SAVE_DATA2: ErrorCode = ErrorCode(906);

    #[inline]
    pub const fn other(code: u32) -> Self {
        ErrorCode(code)
    }

    #[inline]
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PCSError {
    #[serde(rename = "code")]
    pub http_code: u16,
    #[serde(rename = "internal_code")]
    pub tcs_code: ErrorCode,
    #[serde(rename = "error")]
    pub message: String,
}

impl PCSError {
    pub fn bad_request(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self {
            http_code: 400,
            tcs_code: code,
            message: msg.into(),
        }
    }

    pub fn unauthorized(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self {
            http_code: 401,
            tcs_code: code,
            message: msg.into(),
        }
    }

    pub fn forbidden(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self {
            http_code: 403,
            tcs_code: code,
            message: msg.into(),
        }
    }

    pub fn not_found(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self {
            http_code: 404,
            tcs_code: code,
            message: msg.into(),
        }
    }

    pub fn internal_error(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self {
            http_code: 500,
            tcs_code: code,
            message: msg.into(),
        }
    }

    pub fn db_not_found() -> Self {
        Self {
            http_code: 500,
            tcs_code: ErrorCode::DB_NOT_FOUND,
            message: "data not found".into(),
        }
    }

    pub fn db_error(code: ErrorCode, msg: impl Into<String>) -> Self {
        Self {
            http_code: 500,
            tcs_code: code,
            message: msg.into(),
        }
    }
}

impl fmt::Display for PCSError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl core::error::Error for PCSError {}

impl<T: ByteStream> From<PCSError> for Response<T> {
    fn from(err: PCSError) -> Self {
        Response {
            status_code: err.http_code,
            content_type: Some(JSON_CONTENT_TYPE.into()),
            body: Some(Body::Bytes(serde_json::to_vec(&err).unwrap_or_default())),
        }
    }
}
