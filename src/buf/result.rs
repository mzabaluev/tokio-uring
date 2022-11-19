use std::error::Error;
use std::fmt::{self, Debug, Display};
use std::io;

/// A specialized `Result` type for `io-uring` operations with buffers.
///
/// This type is used as a return value for asynchronous `io-uring` methods that
/// require passing ownership of a buffer to the runtime. When the operation
/// completes, the buffer is returned whether or not the operation completed
/// successfully.
///
/// # Examples
///
/// ```no_run
/// use tokio_uring::fs::File;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     tokio_uring::start(async {
///         // Open a file
///         let file = File::open("hello.txt").await?;
///
///         let buf = vec![0; 4096];
///         // Read some data, the buffer is passed by ownership and
///         // submitted to the kernel. When the operation completes,
///         // we get the buffer back.
///         let (res, buf) = file.read_at(buf, 0).await;
///         let n = res?;
///
///         // Display the contents
///         println!("{:?}", &buf[..n]);
///
///         Ok(())
///     })
/// }
/// ```
pub type BufResult<T, B> = Result<(T, B), BufError<B>>;

/// The error type for `io-uring` operations with buffers.
///
/// When the operation fails, the buffer is returned alongside the error code.
#[derive(Debug)]
pub struct BufError<B>(pub io::Error, pub B);

impl<B> From<BufError<B>> for io::Error {
    fn from(e: BufError<B>) -> Self {
        e.0
    }
}

impl<B> Display for BufError<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<B: Debug> Error for BufError<B> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.0.source()
    }
}

/// A trait providing utility methods for [`BufResult`].
pub trait ResultExt {
    type Output;
    type Buf;

    fn lift_buf(self) -> (io::Result<Self::Output>, Self::Buf);

    fn map_buf<B, F>(self, f: F) -> BufResult<Self::Output, B>
    where
        F: FnOnce(Self::Buf) -> B;
}

impl<T, B> ResultExt for BufResult<T, B> {
    type Output = T;
    type Buf = B;

    fn lift_buf(self) -> (io::Result<Self::Output>, Self::Buf) {
        match self {
            Ok((out, buf)) => (Ok(out), buf),
            Err(BufError(e, buf)) => (Err(e), buf),
        }
    }

    fn map_buf<C, F>(self, f: F) -> BufResult<Self::Output, C>
    where
        F: FnOnce(Self::Buf) -> C,
    {
        match self {
            Ok((out, buf)) => Ok((out, f(buf))),
            Err(BufError(e, buf)) => Err(BufError(e, f(buf))),
        }
    }
}
