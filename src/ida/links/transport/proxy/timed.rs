//! Per-operation inactivity deadlines for proxy sockets and tunneled TLS.

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::Sleep;

pub(super) struct Timed<S> {
    inner: S,
    timeout: Duration,
    read: Option<Pin<Box<Sleep>>>,
    write: Option<Pin<Box<Sleep>>>,
}

impl<S> Timed<S> {
    pub(super) fn new(inner: S, timeout: Duration) -> Self {
        Self {
            inner,
            timeout,
            read: None,
            write: None,
        }
    }
}

fn deadline<T>(
    result: Poll<io::Result<T>>,
    timer: &mut Option<Pin<Box<Sleep>>>,
    timeout: Duration,
    cx: &mut Context<'_>,
) -> Poll<io::Result<T>> {
    if result.is_ready() {
        *timer = None;
        return result;
    }
    let timer = timer.get_or_insert_with(|| Box::pin(tokio::time::sleep(timeout)));
    if timer.as_mut().poll(cx).is_ready() {
        Poll::Ready(Err(io::Error::new(io::ErrorKind::TimedOut, "proxy I/O timed out")))
    } else {
        Poll::Pending
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Timed<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_read(cx, buffer);
        let timeout = self.timeout;
        deadline(result, &mut self.read, timeout, cx)
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Timed<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, bytes);
        let timeout = self.timeout;
        deadline(result, &mut self.write, timeout, cx)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_flush(cx);
        let timeout = self.timeout;
        deadline(result, &mut self.write, timeout, cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_shutdown(cx);
        let timeout = self.timeout;
        deadline(result, &mut self.write, timeout, cx)
    }
}
