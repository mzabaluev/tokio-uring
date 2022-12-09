//! Thread-safe fixed buffers.

mod handle;
pub use handle::FixedBuf;

pub mod pool;
pub use pool::FixedBufPool;

mod registry;
pub use registry::FixedBufRegistry;
