use http::Response;
use serde::Serialize;

use crate::{types::error::PCSError, utils::MapPCSError};

pub(crate) fn ok<T: Serialize>(body: &T) -> Result<Response<Vec<u8>>, PCSError> {
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(serde_json::to_vec(body).map_internal_err()?)
        .unwrap())
}

pub(crate) fn created<T: Serialize>(body: &T) -> Result<Response<Vec<u8>>, PCSError> {
    Ok(Response::builder()
        .status(201)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(serde_json::to_vec(body).map_internal_err()?)
        .unwrap())
}

pub(crate) fn no_content() -> Result<Response<Vec<u8>>, PCSError> {
    Ok(Response::builder().status(204).body(Vec::new()).unwrap())
}
