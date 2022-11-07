use crate::buf::{BoundedBuf, BoundedBufMut, IoBuf, IoBufMut, Slice};
use crate::driver::{Op, SharedFd};
use crate::fs::OpenOptions;

use std::fmt;
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::path::Path;

/// A reference to an open file on the filesystem.
///
/// An instance of a `File` can be read and/or written depending on what options
/// it was opened with. The `File` type provides **positional** read and write
/// operations. The file does not maintain an internal cursor. The caller is
/// required to specify an offset when issuing an operation.
///
/// While files are automatically closed when they go out of scope, the
/// operation happens asynchronously in the background. It is recommended to
/// call the `close()` function in order to guarantee that the file successfully
/// closed before exiting the scope. Closing a file does not guarantee writes
/// have persisted to disk. Use [`sync_all`] to ensure all writes have reached
/// the filesystem.
///
/// [`sync_all`]: File::sync_all
///
/// # Examples
///
/// Creates a new file and write data to it:
///
/// ```no_run
/// use tokio_uring::fs::File;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     tokio_uring::start(async {
///         // Open a file
///         let file = File::create("hello.txt").await?;
///
///         // Write some data
///         let (res, buf) = file.write_at(&b"hello world"[..], 0).await;
///         let n = res?;
///
///         println!("wrote {} bytes", n);
///
///         // Sync data to the file system.
///         file.sync_all().await?;
///
///         // Close the file
///         file.close().await?;
///
///         Ok(())
///     })
/// }
/// ```
pub struct File {
    /// Open file descriptor
    fd: SharedFd,
}

impl File {
    /// Attempts to open a file in read-only mode.
    ///
    /// See the [`OpenOptions::open`] method for more details.
    ///
    /// # Errors
    ///
    /// This function will return an error if `path` does not already exist.
    /// Other errors may also be returned according to [`OpenOptions::open`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let f = File::open("foo.txt").await?;
    ///
    ///         // Close the file
    ///         f.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    pub async fn open(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new().read(true).open(path).await
    }

    /// Opens a file in write-only mode.
    ///
    /// This function will create a file if it does not exist,
    /// and will truncate it if it does.
    ///
    /// See the [`OpenOptions::open`] function for more details.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let f = File::create("foo.txt").await?;
    ///
    ///         // Close the file
    ///         f.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    pub async fn create(path: impl AsRef<Path>) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .await
    }

    pub(crate) fn from_shared_fd(fd: SharedFd) -> File {
        File { fd }
    }

    /// Converts a [`std::fs::File`][std] to a [`tokio_uring::fs::File`][file].
    ///
    /// [std]: std::fs::File
    /// [file]: File
    pub fn from_std(file: std::fs::File) -> File {
        File::from_shared_fd(SharedFd::new(file.into_raw_fd()))
    }

    /// Read some bytes at the specified offset from the file into the specified
    /// buffer, returning how many bytes were read.
    ///
    /// # Return
    ///
    /// The method returns the operation result and the same buffer value passed
    /// as an argument.
    ///
    /// If the method returns [`Ok(n)`], then the read was successful. A nonzero
    /// `n` value indicates that the buffer has been filled with `n` bytes of
    /// data from the file. If `n` is `0`, then one of the following happened:
    ///
    /// 1. The specified offset is the end of the file.
    /// 2. The buffer specified was 0 bytes in length.
    ///
    /// It is not an error if the returned value `n` is smaller than the buffer
    /// size, even when the file contains enough data to fill the buffer.
    ///
    /// # Errors
    ///
    /// If this function encounters any form of I/O or other error, an error
    /// variant will be returned. The buffer is returned on error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let f = File::open("foo.txt").await?;
    ///         let buffer = vec![0; 10];
    ///
    ///         // Read up to 10 bytes
    ///         let (res, buffer) = f.read_at(buffer, 0).await;
    ///         let n = res?;
    ///
    ///         println!("The bytes: {:?}", &buffer[..n]);
    ///
    ///         // Close the file
    ///         f.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    pub async fn read_at<T: BoundedBufMut>(&self, buf: T, pos: u64) -> crate::BufResult<usize, T> {
        // Submit the read operation
        let op = Op::read_at(&self.fd, buf, pos).unwrap();
        op.await
    }

    /// Read some bytes at the specified offset from the file into the specified
    /// array of buffers, returning how many bytes were read.
    ///
    /// # Return
    ///
    /// The method returns the operation result and the same array of buffers
    /// passed as an argument.
    ///
    /// If the method returns [`Ok(n)`], then the read was successful. A nonzero
    /// `n` value indicates that the buffers have been filled with `n` bytes of
    /// data from the file. If `n` is `0`, then one of the following happened:
    ///
    /// 1. The specified offset is the end of the file.
    /// 2. The buffers specified were 0 bytes in length.
    ///
    /// It is not an error if the returned value `n` is smaller than the buffer
    /// size, even when the file contains enough data to fill the buffer.
    ///
    /// # Errors
    ///
    /// If this function encounters any form of I/O or other error, an error
    /// variant will be returned. The buffer is returned on error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let f = File::open("foo.txt").await?;
    ///         let buffers = vec![Vec::<u8>::with_capacity(10), Vec::<u8>::with_capacity(10)];
    ///
    ///         // Read up to 20 bytes
    ///         let (res, buffer) = f.readv_at(buffers, 0).await;
    ///         let n = res?;
    ///
    ///         println!("Read {} bytes", n);
    ///
    ///         // Close the file
    ///         f.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    pub async fn readv_at<T: IoBufMut>(
        &self,
        bufs: Vec<T>,
        pos: u64,
    ) -> crate::BufResult<usize, Vec<T>> {
        // Submit the read operation
        let op = Op::readv_at(&self.fd, bufs, pos).unwrap();
        op.await
    }

    /// Write data from buffers into this file at the specified offset,
    /// returning how many bytes were written.
    ///
    /// This function will attempt to write the entire contents of `bufs`, but
    /// the entire write may not succeed, or the write may also generate an
    /// error. The bytes will be written starting at the specified offset.
    ///
    /// # Return
    ///
    /// The method returns the operation result and the same array of buffers passed
    /// in as an argument. A return value of `0` typically means that the
    /// underlying file is no longer able to accept bytes and will likely not be
    /// able to in the future as well, or that the buffer provided is empty.
    ///
    /// # Errors
    ///
    /// Each call to `write` may generate an I/O error indicating that the
    /// operation could not be completed. If an error is returned then no bytes
    /// in the buffer were written to this writer.
    ///
    /// It is **not** considered an error if the entire buffer could not be
    /// written to this writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let file = File::create("foo.txt").await?;
    ///
    ///         // Writes some prefix of the byte string, not necessarily all of it.
    ///         let bufs = vec!["some".to_owned().into_bytes(), " bytes".to_owned().into_bytes()];
    ///         let (res, _) = file.writev_at(bufs, 0).await;
    ///         let n = res?;
    ///
    ///         println!("wrote {} bytes", n);
    ///
    ///         // Close the file
    ///         file.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    ///
    /// [`Ok(n)`]: Ok
    pub async fn writev_at<T: IoBuf>(
        &self,
        buf: Vec<T>,
        pos: u64,
    ) -> crate::BufResult<usize, Vec<T>> {
        let op = Op::writev_at(&self.fd, buf, pos).unwrap();
        op.await
    }

    /// Read the exact number of bytes required to fill `buf` at the specified
    /// offset from the file.
    ///
    /// This function reads as many as bytes as necessary to completely fill the
    /// specified buffer `buf`.
    ///
    /// # Return
    ///
    /// The method returns the operation result and the same buffer value passed
    /// as an argument.
    ///
    /// If the method returns [`Ok(())`], then the read was successful.
    ///
    /// # Errors
    ///
    /// If this function encounters an "end of file" before completely filling
    /// the buffer, it returns an error of the kind [`ErrorKind::UnexpectedEof`].
    /// The buffer is returned on error.
    ///
    /// If this function encounters any form of I/O or other error, an error
    /// variant will be returned. The buffer is returned on error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let f = File::open("foo.txt").await?;
    ///         let buffer = Vec::with_capacity(10);
    ///
    ///         // Read up to 10 bytes
    ///         let (res, buffer) = f.read_exact_at(buffer, 0).await;
    ///         res?;
    ///
    ///         println!("The bytes: {:?}", buffer);
    ///
    ///         // Close the file
    ///         f.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    ///
    /// [`ErrorKind::UnexpectedEof`]: std::io::ErrorKind::UnexpectedEof
    pub async fn read_exact_at<T>(&self, buf: T, pos: u64) -> crate::BufResult<(), T>
    where
        T: BoundedBufMut,
    {
        let orig_bounds = buf.bounds();
        let (res, buf) = self.read_exact_slice_at(buf.slice(..), pos).await;
        (res, T::from_buf_bounds(buf, orig_bounds))
    }

    async fn read_exact_slice_at<T: IoBufMut>(
        &self,
        mut buf: Slice<T>,
        mut pos: u64,
    ) -> crate::BufResult<(), T> {
        if pos.checked_add(buf.bytes_total() as u64).is_none() {
            return (
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "buffer too large for file",
                )),
                buf.into_inner(),
            );
        }

        while buf.bytes_total() != 0 {
            let (res, slice) = self.read_at(buf, pos).await;
            match res {
                Ok(0) => {
                    return (
                        Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "failed to fill whole buffer",
                        )),
                        slice.into_inner(),
                    )
                }
                Ok(n) => {
                    pos += n as u64;
                    buf = slice.slice(n..);
                }

                // No match on an EINTR error is performed because this
                // crate's design ensures we are not calling the 'wait' option
                // in the ENTER syscall. Only an Enter with 'wait' can generate
                // an EINTR according to the io_uring man pages.
                Err(e) => return (Err(e), slice.into_inner()),
            };
        }

        (Ok(()), buf.into_inner())
    }

    /// Write a buffer into this file at the specified offset, returning how
    /// many bytes were written.
    ///
    /// This function will attempt to write the entire contents of `buf`, but
    /// the entire write may not succeed, or the write may also generate an
    /// error. The bytes will be written starting at the specified offset.
    ///
    /// # Return
    ///
    /// The method returns the operation result and the same buffer value passed
    /// in as an argument. A return value of `0` typically means that the
    /// underlying file is no longer able to accept bytes and will likely not be
    /// able to in the future as well, or that the buffer provided is empty.
    ///
    /// # Errors
    ///
    /// Each call to `write` may generate an I/O error indicating that the
    /// operation could not be completed. If an error is returned then no bytes
    /// in the buffer were written to this writer.
    ///
    /// It is **not** considered an error if the entire buffer could not be
    /// written to this writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let file = File::create("foo.txt").await?;
    ///
    ///         // Writes some prefix of the byte string, not necessarily all of it.
    ///         let (res, _) = file.write_at(&b"some bytes"[..], 0).await;
    ///         let n = res?;
    ///
    ///         println!("wrote {} bytes", n);
    ///
    ///         // Close the file
    ///         file.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    ///
    /// [`Ok(n)`]: Ok
    pub async fn write_at<T: BoundedBuf>(&self, buf: T, pos: u64) -> crate::BufResult<usize, T> {
        let op = Op::write_at(&self.fd, buf, pos).unwrap();
        op.await
    }

    /// Attempts to write an entire buffer into this file at the specified offset.
    ///
    /// This method will continuously call [`write_at`] until there is no more data
    /// to be written or an error is returned.
    /// This method will not return until the entire buffer has been successfully
    /// written or an error occurs.
    ///
    /// If the buffer contains no data, this will never call [`write_at`].
    ///
    /// # Return
    ///
    /// The method returns the operation result and the same buffer value passed
    /// in as an argument.
    ///
    /// # Errors
    ///
    /// This function will return the first error that [`write_at`] returns.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let file = File::create("foo.txt").await?;
    ///
    ///         // Writes some prefix of the byte string, not necessarily all of it.
    ///         let (res, _) = file.write_all_at(&b"some bytes"[..], 0).await;
    ///         res?;
    ///
    ///         println!("wrote all bytes");
    ///
    ///         // Close the file
    ///         file.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    ///
    /// [`write_at`]: File::write_at
    pub async fn write_all_at<T>(&self, buf: T, pos: u64) -> crate::BufResult<(), T>
    where
        T: BoundedBuf,
    {
        let orig_bounds = buf.bounds();
        let (res, buf) = self.write_all_slice_at(buf.slice(..), pos).await;
        (res, T::from_buf_bounds(buf, orig_bounds))
    }

    async fn write_all_slice_at<T: IoBuf>(
        &self,
        mut buf: Slice<T>,
        mut pos: u64,
    ) -> crate::BufResult<(), T> {
        if pos.checked_add(buf.bytes_init() as u64).is_none() {
            return (
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "buffer too large for file",
                )),
                buf.into_inner(),
            );
        }

        while buf.bytes_init() != 0 {
            let (res, slice) = self.write_at(buf, pos).await;
            match res {
                Ok(0) => {
                    return (
                        Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to write whole buffer",
                        )),
                        slice.into_inner(),
                    )
                }
                Ok(n) => {
                    pos += n as u64;
                    buf = slice.slice(n..);
                }

                // No match on an EINTR error is performed because this
                // crate's design ensures we are not calling the 'wait' option
                // in the ENTER syscall. Only an Enter with 'wait' can generate
                // an EINTR according to the io_uring man pages.
                Err(e) => return (Err(e), slice.into_inner()),
            };
        }

        (Ok(()), buf.into_inner())
    }

    /// Attempts to sync all OS-internal metadata to disk.
    ///
    /// This function will attempt to ensure that all in-memory data reaches the
    /// filesystem before completing.
    ///
    /// This can be used to handle errors that would otherwise only be caught
    /// when the `File` is closed.  Dropping a file will ignore errors in
    /// synchronizing this in-memory data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let f = File::create("foo.txt").await?;
    ///         let (res, buf) = f.write_at(&b"Hello, world!"[..], 0).await;
    ///         let n = res?;
    ///
    ///         f.sync_all().await?;
    ///
    ///         // Close the file
    ///         f.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    pub async fn sync_all(&self) -> io::Result<()> {
        Op::fsync(&self.fd)?.await
    }

    /// Attempts to sync file data to disk.
    ///
    /// This method is similar to [`sync_all`], except that it may not
    /// synchronize file metadata to the filesystem.
    ///
    /// This is intended for use cases that must synchronize content, but don't
    /// need the metadata on disk. The goal of this method is to reduce disk
    /// operations.
    ///
    /// Note that some platforms may simply implement this in terms of
    /// [`sync_all`].
    ///
    /// [`sync_all`]: File::sync_all
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         let f = File::create("foo.txt").await?;
    ///         let (res, buf) = f.write_at(&b"Hello, world!"[..], 0).await;
    ///         let n = res?;
    ///
    ///         f.sync_data().await?;
    ///
    ///         // Close the file
    ///         f.close().await?;
    ///         Ok(())
    ///     })
    /// }
    /// ```
    pub async fn sync_data(&self) -> io::Result<()> {
        Op::datasync(&self.fd)?.await
    }

    /// Closes the file.
    ///
    /// The method completes once the close operation has completed,
    /// guaranteeing that resources associated with the file have been released.
    ///
    /// If `close` is not called before dropping the file, the file is closed in
    /// the background, but there is no guarantee as to **when** the close
    /// operation will complete.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use tokio_uring::fs::File;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     tokio_uring::start(async {
    ///         // Open the file
    ///         let f = File::open("foo.txt").await?;
    ///         // Close the file
    ///         f.close().await?;
    ///
    ///         Ok(())
    ///     })
    /// }
    /// ```
    pub async fn close(self) -> io::Result<()> {
        self.fd.close().await;
        Ok(())
    }
}

impl FromRawFd for File {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        File::from_shared_fd(SharedFd::new(fd))
    }
}

impl AsRawFd for File {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("File")
            .field("fd", &self.fd.raw_fd())
            .finish()
    }
}

/// Removes a File
///
/// # Examples
///
/// ```no_run
/// use tokio_uring::fs::remove_file;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     tokio_uring::start(async {
///         remove_file("/some/file.txt").await?;
///         Ok::<(), std::io::Error>(())
///     })?;
///     Ok(())
/// }
/// ```
pub async fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    Op::unlink_file(path.as_ref())?.await
}

/// Renames a file or directory to a new name, replacing the original file if
/// `to` already exists.
///
/// This will not work if the new name is on a different mount point.
///
/// # Example
///
/// ```no_run
/// use tokio_uring::fs::rename;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     tokio_uring::start(async {
///         rename("a.txt", "b.txt").await?; // Rename a.txt to b.txt
///         Ok::<(), std::io::Error>(())
///     })?;
///     Ok(())
/// }
/// ```
pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    Op::rename_at(from.as_ref(), to.as_ref(), 0)?.await
}
