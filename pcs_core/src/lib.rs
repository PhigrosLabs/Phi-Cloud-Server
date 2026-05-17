pub mod file;
pub mod game;
pub mod handler;
pub mod types;
pub mod user;
pub(crate) mod utils;

use bytes::Bytes;
use futures::{Stream, StreamExt};
use http_body::Frame;
use http_body_util::{BodyExt, StreamBody};
use std::convert::Infallible;

pub type PcsBody = http_body_util::combinators::BoxBody<Bytes, Infallible>;

pub fn pcs_body_from_bytes(data: impl Into<Bytes>) -> PcsBody {
    BodyExt::boxed(http_body_util::Full::new(data.into()))
}

pub fn pcs_body_empty() -> PcsBody {
    BodyExt::boxed(http_body_util::Full::new(Bytes::new()))
}

pub fn pcs_body_from_stream<S>(stream: S) -> PcsBody
where
    S: Stream<Item = Bytes> + Sync + Send + 'static,
{
    BodyExt::boxed(StreamBody::new(stream.map(|b| Ok(Frame::data(b)))))
}
