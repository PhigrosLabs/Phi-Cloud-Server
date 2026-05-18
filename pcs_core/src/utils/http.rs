use alloc::boxed::Box;
use bytes::Bytes;
use futures::{Stream, stream};
use http::Response;
use serde::Serialize;

use crate::{
    types::{PcsBody, error::PCSError},
    utils::MapPCSError,
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
    Ok(Response::builder().status(204).body(None).unwrap())
}

pub fn pcs_body_from_bytes(data: impl Into<Bytes>) -> PcsBody {
    let bytes = data.into();
    Some(Box::new(stream::iter(core::iter::once(bytes))))
}

pub fn pcs_body_from_stream<S>(stream: S) -> PcsBody
where
    S: Stream<Item = Bytes> + Send + Unpin + 'static,
{
    Some(Box::new(stream))
}
