use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::Future;
use futures::stream::StreamExt;
use pcs_core::types::PcsBody;
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

pub async fn build_response(resp: http::Response<PcsBody>) -> worker::Result<Response> {
    let (parts, body) = resp.into_parts();

    let headers = Headers::new();
    for (key, value) in parts.headers.iter() {
        headers.set(
            key.as_str(),
            value
                .to_str()
                .map_err(|e| worker::Error::RustError(e.to_string()))?,
        )?;
    }

    let status_code = parts.status.as_u16();

    let response = if let Some(body) = body {
        let stream = body.map(|bytes| -> worker::Result<Vec<u8>> { Ok(bytes.to_vec()) });
        ResponseBuilder::new()
            .with_status(status_code)
            .with_headers(headers)
            .from_stream(stream)?
    } else {
        ResponseBuilder::new()
            .with_status(status_code)
            .with_headers(headers)
            .empty()
    };

    Ok(response)
}
