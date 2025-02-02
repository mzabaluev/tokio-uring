use std::future::Future;
use std::io;
use std::mem::ManuallyDrop;
use std::os::unix::io::AsRawFd;
use tokio::io::unix::AsyncFd;
use tokio::task::LocalSet;

mod context;
pub(crate) mod driver;

pub(crate) use context::RuntimeContext;

thread_local! {
    pub(crate) static CONTEXT: RuntimeContext = RuntimeContext::new();
}

/// The Runtime executor
pub struct Runtime {
    /// LocalSet for !Send tasks
    local: ManuallyDrop<LocalSet>,

    /// Strong reference to the driver.
    driver: driver::Handle,

    /// Tokio runtime, always current-thread
    rt: ManuallyDrop<tokio::runtime::Runtime>,
}

/// Spawns a new asynchronous task, returning a [`JoinHandle`] for it.
///
/// Spawning a task enables the task to execute concurrently to other tasks.
/// There is no guarantee that a spawned task will execute to completion. When a
/// runtime is shutdown, all outstanding tasks are dropped, regardless of the
/// lifecycle of that task.
///
/// This function must be called from the context of a `tokio-uring` runtime.
///
/// [`JoinHandle`]: tokio::task::JoinHandle
///
/// # Examples
///
/// In this example, a server is started and `spawn` is used to start a new task
/// that processes each received connection.
///
/// ```no_run
/// tokio_uring::start(async {
///     let handle = tokio_uring::spawn(async {
///         println!("hello from a background task");
///     });
///
///     // Let the task complete
///     handle.await.unwrap();
/// });
/// ```
pub fn spawn<T: Future + 'static>(task: T) -> tokio::task::JoinHandle<T::Output> {
    tokio::task::spawn_local(task)
}

impl Runtime {
    /// Create a new tokio_uring runtime on the current thread
    pub fn new(b: &crate::Builder) -> io::Result<Runtime> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .on_thread_park(|| {
                CONTEXT.with(|x| {
                    let _ = x
                        .handle()
                        .expect("Internal error, driver context not present when invoking hooks")
                        .flush();
                });
            })
            .enable_all()
            .build()?;

        let rt = ManuallyDrop::new(rt);

        let local = ManuallyDrop::new(LocalSet::new());

        let driver = driver::Handle::new(b)?;

        let driver_fd = driver.as_raw_fd();

        let drive = {
            let _guard = rt.enter();
            let driver = AsyncFd::new(driver_fd).unwrap();

            async move {
                loop {
                    // Wait for read-readiness
                    let mut guard = driver.readable().await.unwrap();
                    CONTEXT.with(|cx| cx.with_handle_mut(|driver| driver.tick()));
                    guard.clear_ready();
                }
            }
        };

        local.spawn_local(drive);

        Ok(Runtime { local, rt, driver })
    }

    /// Runs a future to completion on the current runtime
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        struct ContextGuard;

        impl Drop for ContextGuard {
            fn drop(&mut self) {
                CONTEXT.with(|cx| cx.unset_driver());
            }
        }

        CONTEXT.with(|cx| cx.set_handle(self.driver.clone()));

        let _guard = ContextGuard;

        tokio::pin!(future);

        let res = self
            .rt
            .block_on(self.local.run_until(std::future::poll_fn(|cx| {
                // assert!(drive.as_mut().poll(cx).is_pending());
                future.as_mut().poll(cx)
            })));

        res
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // drop tasks
        unsafe {
            ManuallyDrop::drop(&mut self.local);
            ManuallyDrop::drop(&mut self.rt);
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::builder;

    #[test]
    fn block_on() {
        let rt = Runtime::new(&builder()).unwrap();
        rt.block_on(async move { () });
    }

    #[test]
    fn block_on_twice() {
        let rt = Runtime::new(&builder()).unwrap();
        rt.block_on(async move { () });
        rt.block_on(async move { () });
    }
}
