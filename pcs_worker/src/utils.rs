use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;
use futures::stream::StreamExt;
use pcs_core::types::{Body, ByteStream, Response as PcsResponse};
use worker::{Headers, Response, ResponseBuilder};

pub struct UnsafeSend<F>(pub F);

unsafe impl<F> Send for UnsafeSend<F> {}

impl<F: Future> Future for UnsafeSend<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            let inner = self.map_unchecked_mut(|s| &mut s.0);
            inner.poll(cx)
        }
    }
}

pub struct UnsafeStream<S>(pub S);

unsafe impl<S> Send for UnsafeStream<S> {}
unsafe impl<S> Sync for UnsafeStream<S> {}

impl<S: futures::Stream> futures::Stream for UnsafeStream<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unsafe {
            let inner = self.map_unchecked_mut(|s| &mut s.0);
            inner.poll_next(cx)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub async fn build_response<T: ByteStream>(resp: PcsResponse<T>) -> worker::Result<Response> {
    let PcsResponse {
        status_code,
        content_type,
        body,
    } = resp;

    let headers = Headers::new();
    if let Some(ct) = &content_type {
        headers.set("Content-Type", ct.as_str())?;
    }

    let response = match body {
        Some(Body::Bytes(bytes)) => ResponseBuilder::new()
            .with_status(status_code)
            .with_headers(headers)
            .body(worker::ResponseBody::Body(bytes.to_vec())),
        Some(Body::ByteStream(stream)) => ResponseBuilder::new()
            .with_status(status_code)
            .with_headers(headers)
            .from_stream(
                stream.map(|item| item.map_err(|e| worker::Error::RustError(e.to_string()))),
            )?,
        None => ResponseBuilder::new()
            .with_status(status_code)
            .with_headers(headers)
            .empty(),
    };

    Ok(response)
}
