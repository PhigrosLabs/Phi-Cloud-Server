use alloc::string::ToString;

use crate::types::error::{ErrorCode, PCSError};

pub(crate) trait MapPCSError<T> {
    fn map_pcs_error(self, code: ErrorCode) -> Result<T, PCSError>;
    fn map_pcs_bad(self, code: ErrorCode) -> Result<T, PCSError>;
}

impl<T, E: core::error::Error> MapPCSError<T> for Result<T, E> {
    fn map_pcs_error(self, code: ErrorCode) -> Result<T, PCSError> {
        self.map_err(|e| PCSError::internal_error(code, e.to_string()))
    }
    fn map_pcs_bad(self, code: ErrorCode) -> Result<T, PCSError> {
        self.map_err(|e| PCSError::bad_request(code, e.to_string()))
    }
}
