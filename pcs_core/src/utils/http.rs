use serde::Serialize;

use crate::{
    types::{Body, ByteStream, JSON_CONTENT_TYPE, Response, error::PCSError},
    utils::MapPCSError,
};

pub(crate) fn ok<T: Serialize, S: ByteStream>(body: &T) -> Result<Response<S>, PCSError> {
    Ok(Response {
        status_code: 200,
        content_type: Some(JSON_CONTENT_TYPE.into()),
        body: body_from_serialize(body)?,
    })
}

fn body_from_serialize<T: Serialize, S: ByteStream>(body: &T) -> Result<Option<Body<S>>, PCSError> {
    Ok(Some(Body::Bytes(
        serde_json::to_vec(body).map_internal_err()?,
    )))
}

pub(crate) fn created<T: Serialize, S: ByteStream>(body: &T) -> Result<Response<S>, PCSError> {
    Ok(Response {
        status_code: 201,
        content_type: Some(JSON_CONTENT_TYPE.into()),
        body: body_from_serialize(body)?,
    })
}

pub(crate) fn no_content<S: ByteStream>() -> Result<Response<S>, PCSError> {
    Ok(Response {
        status_code: 204,
        content_type: None,
        body: None,
    })
}
