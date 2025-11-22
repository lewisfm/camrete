use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use pin_project::pin_project;
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

pub trait AsyncReadExt: Sized {
    fn progress<F>(self, f: F) -> ProgressReader<Self, F>
    where
        F: FnMut(u64);
}

impl<T: AsyncRead> AsyncReadExt for T {
    fn progress<F>(self, f: F) -> ProgressReader<Self, F>
    where
        F: FnMut(u64),
    {
        ProgressReader {
            reader: self,
            bytes_read: 0,
            on_progress: f,
        }
    }
}

#[pin_project]
pub struct ProgressReader<R, F> {
    #[pin]
    reader: R,
    bytes_read: u64,
    on_progress: F,
}

impl<R: AsyncRead, F: FnMut(u64)> AsyncRead for ProgressReader<R, F> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        let before = buf.filled().len();

        let outcome = this.reader.poll_read(cx, buf);

        let after = buf.filled().len();
        let change = after - before;
        if change != 0 {
            *this.bytes_read += change as u64;
            let bytes = *this.bytes_read;
            (this.on_progress)(bytes);
        }

        outcome
    }
}

impl<R: AsyncBufRead, F: FnMut(u64)> AsyncBufRead for ProgressReader<R, F> {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        self.project().reader.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = self.project();
        *this.bytes_read += amt as u64;

        let bytes = *this.bytes_read;
        (this.on_progress)(bytes);

        this.reader.consume(amt)
    }
}
