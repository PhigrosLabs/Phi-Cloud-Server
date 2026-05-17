use http::Response;
use serde::Serialize;

use crate::{
    PcsBody, pcs_body_empty, pcs_body_from_bytes, types::error::PCSError, utils::MapPCSError,
};

pub(crate) fn ok<T: Serialize>(body: &T) -> Result<Response<PcsBody>, PCSError> {
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(pcs_body_from_bytes(
            serde_json::to_vec(body).map_internal_err()?,
        ))
        .unwrap())
}

pub(crate) fn created<T: Serialize>(body: &T) -> Result<Response<PcsBody>, PCSError> {
    Ok(Response::builder()
        .status(201)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(pcs_body_from_bytes(
            serde_json::to_vec(body).map_internal_err()?,
        ))
        .unwrap())
}

pub(crate) fn no_content() -> Result<Response<PcsBody>, PCSError> {
    Ok(Response::builder()
        .status(204)
        .body(pcs_body_empty())
        .unwrap())
}
