//! Thread-safe fixed buffers.

mod handle;
pub use handle::FixedBuf;

mod registry;
pub use registry::FixedBufRegistry;
