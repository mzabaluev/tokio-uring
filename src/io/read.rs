use crate::buf::BoundedBufMut;
use crate::io::SharedFd;
use crate::BufResult;

use crate::runtime::driver::op::{Completable, CqeResult, Op};
use std::io;

pub(crate) struct Read<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(dead_code)]
    fd: SharedFd,

    /// Reference to the in-flight buffer.
    pub(crate) buf: T,
}

impl<T: BoundedBufMut> Op<Read<T>> {
    pub(crate) fn read_at(fd: &SharedFd, buf: T, offset: u64) -> io::Result<Op<Read<T>>> {
        use io_uring::{opcode, types};

        Op::submit_with(
            Read {
                fd: fd.clone(),
                buf,
            },
            |read| {
                // Get raw buffer info
                let ptr = read.buf.stable_mut_ptr();
                let len = read.buf.bytes_total();
                opcode::Read::new(types::Fd(fd.raw_fd()), ptr, len as _)
                    .offset(offset as _)
                    .build()
            },
        )
    }
}

impl<T> Completable for Read<T>
where
    T: BoundedBufMut,
{
    type Output = BufResult<usize, T>;

    fn complete(self, cqe: CqeResult) -> Self::Output {
        cqe.buf_result_with(self.buf, |v, mut buf| {
            // Convert the operation result to `usize`
            let n = v as usize;

            // Advance the initialized cursor.
            // Safety: the kernel wrote `n` bytes to the buffer.
            unsafe {
                buf.set_init(n);
            }

            (n, buf)
        })
    }
}
