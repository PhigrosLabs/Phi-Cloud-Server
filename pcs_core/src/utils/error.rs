use crate::types::error::PCSError;

pub(crate) trait MapPCSError<T> {
    fn map_internal_err(self) -> Result<T, PCSError>;
    fn map_db_err(self) -> Result<T, PCSError>;
    fn map_bad_err(self) -> Result<T, PCSError>;
}

impl<T, E: std::error::Error> MapPCSError<T> for Result<T, E> {
    fn map_db_err(self) -> Result<T, PCSError> {
        self.map_err(|e| PCSError::db_error(e.to_string()))
    }
    fn map_internal_err(self) -> Result<T, PCSError> {
        self.map_err(|e| PCSError::internal_error(e.to_string()))
    }
    fn map_bad_err(self) -> Result<T, PCSError> {
        self.map_err(|e| PCSError::bad_request(e.to_string()))
    }
}
