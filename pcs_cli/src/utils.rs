use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct UnsafeSendFuture<F>(pub F);

unsafe impl<F> Send for UnsafeSendFuture<F> {}

impl<F: Future> Future for UnsafeSendFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            let this = self.get_unchecked_mut();

            Pin::new_unchecked(&mut this.0).poll(cx)
        }
    }
}
