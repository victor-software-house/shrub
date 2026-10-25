//! Tasks the shell runs concurrently with itself.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};

use futures::FutureExt as _;
use futures::future::{AbortHandle, Abortable, Aborted};
use tokio::sync::Notify;

use crate::{error, sys};

/// A future that yields a task's output when it finishes.
type TaskFuture<T> = Pin<Box<dyn Future<Output = Result<T, error::Error>> + Send + Sync>>;

/// Tracks a task started on the shell's behalf. Awaiting it yields the task's
/// output, or an error if the task couldn't run to completion (e.g., it
/// panicked).
///
/// Dropping the handle doesn't stop the task.
pub struct TaskHandle<T> {
    future: TaskFuture<T>,
}

impl<T> TaskHandle<T> {
    /// Tracks a task through a future that yields its output when it finishes.
    ///
    /// # Arguments
    ///
    /// * `future` - The future to await for the task's output.
    pub(crate) fn from_future(
        future: impl Future<Output = Result<T, error::Error>> + Send + Sync + 'static,
    ) -> Self {
        Self {
            future: Box::pin(future),
        }
    }
}

impl<T> Future for TaskHandle<T> {
    type Output = Result<T, error::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.future.as_mut().poll(cx)
    }
}

/// Every task a shell and its subshells start.
///
/// The tasks are background jobs, coprocesses, pipeline stages, and command
/// and process substitutions. An embedder that stops a command takes the
/// handle before running it, then aborts its tasks and waits for them, so the
/// command starts nothing more: dropping the command's future stops only its
/// foreground part, and its tasks run on.
///
/// A subshell shares its parent's handle, so aborting it reaches nested
/// tasks too; [`crate::Shell::fork_state`] gives the copy a handle of its own.
#[derive(Clone, Default)]
pub struct ShellTasks {
    inner: Arc<TasksInner>,
}

#[derive(Default)]
struct TasksInner {
    state: Mutex<TasksState>,
    idle: Notify,
}

#[derive(Default)]
struct TasksState {
    /// Set by [`ShellTasks::abort_and_wait`]: a task started afterwards ends
    /// before it runs.
    aborted: bool,
    /// The abort handle of each task that has not ended, by id.
    running: HashMap<u64, AbortHandle>,
    next_id: u64,
}

impl ShellTasks {
    /// Starts running `future` concurrently with the caller, as one of these
    /// tasks.
    pub(crate) fn spawn<F>(&self, future: F) -> TaskHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        sys::tasks::spawn(self.track(future)).flatten_aborted()
    }

    /// Starts running `future`, which may block the thread it runs on, as one
    /// of these tasks; see [`sys::tasks::spawn_blocking`].
    pub(crate) fn spawn_blocking<F>(
        &self,
        future: F,
    ) -> impl Future<Output = TaskHandle<F::Output>> + use<F>
    where
        F: Future + Send + 'static,
        F::Output: Send + Sync + 'static,
    {
        sys::tasks::spawn_blocking(self.track(future)).map(TaskHandle::flatten_aborted)
    }

    /// Aborts every task still running and every one started later, and
    /// completes once each running one has ended: its future is dropped at
    /// its next await, so it starts nothing more. A task inside a blocking
    /// call ends when the call returns.
    pub fn abort_and_wait(&self) -> impl Future<Output = ()> + Send + 'static {
        let inner = Arc::clone(&self.inner);
        async move {
            loop {
                let idle = inner.idle.notified();
                {
                    let mut state = inner.lock();
                    state.aborted = true;
                    if state.running.is_empty() {
                        return;
                    }
                    for handle in state.running.values() {
                        handle.abort();
                    }
                }
                idle.await;
            }
        }
    }

    /// `future`, ended at once when these tasks are aborted, and counted as
    /// running from now until it ends or is dropped.
    fn track<F: Future>(
        &self,
        future: F,
    ) -> impl Future<Output = Result<F::Output, Aborted>> + use<F> {
        let (handle, registration) = AbortHandle::new_pair();
        let id = {
            let mut state = self.inner.lock();
            if state.aborted {
                handle.abort();
            }
            let id = state.next_id;
            state.next_id += 1;
            state.running.insert(id, handle);
            id
        };
        let ended = Ended {
            inner: Arc::clone(&self.inner),
            id,
        };
        async move {
            let _ended = ended;
            Abortable::new(future, registration).await
        }
    }
}

impl TasksInner {
    /// The state's lock. A task that panicked under it left the state as
    /// it was, whole, so a poisoned lock is taken as it is.
    fn lock(&self) -> std::sync::MutexGuard<'_, TasksState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Removes a task from the running ones when its future ends or is dropped.
struct Ended {
    inner: Arc<TasksInner>,
    id: u64,
}

impl Drop for Ended {
    fn drop(&mut self) {
        let mut state = self.inner.lock();
        state.running.remove(&self.id);
        if state.running.is_empty() {
            self.inner.idle.notify_waiters();
        }
    }
}

impl<T> TaskHandle<Result<T, Aborted>> {
    /// The task's output, with an abort as the task's error.
    fn flatten_aborted(self) -> TaskHandle<T>
    where
        T: Send + 'static,
    {
        TaskHandle::from_future(self.map(|result| {
            result.and_then(|output| {
                output.map_err(|aborted| error::ErrorKind::TaskError(Box::new(aborted)).into())
            })
        }))
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn, reason = "assertions in a fallible test")]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    /// A running task ends at its next await once its tasks are aborted, and
    /// the abort completes only after it has; a task started afterwards
    /// never runs.
    #[tokio::test]
    async fn abort_and_wait_ends_running_tasks_and_refuses_later_ones() {
        let tasks = ShellTasks::default();
        let ticks = Arc::new(AtomicU64::new(0));
        let running = tasks.spawn({
            let ticks = Arc::clone(&ticks);
            async move {
                while ticks.fetch_add(1, Ordering::SeqCst) < u64::MAX {
                    tokio::task::yield_now().await;
                }
            }
        });
        while ticks.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }

        tasks.abort_and_wait().await;
        let seen = ticks.load(Ordering::SeqCst);
        tokio::task::yield_now().await;
        assert_eq!(
            ticks.load(Ordering::SeqCst),
            seen,
            "the task ticked after the abort"
        );
        assert!(running.await.is_err(), "an aborted task yields an error");

        let ran = Arc::new(AtomicU64::new(0));
        let later = tasks.spawn({
            let ran = Arc::clone(&ran);
            async move {
                ran.fetch_add(1, Ordering::SeqCst);
            }
        });
        assert!(later.await.is_err(), "a task started after the abort ends");
        assert_eq!(
            ran.load(Ordering::SeqCst),
            0,
            "a task started after the abort never runs"
        );
    }

    /// Aborting tasks that are all done completes at once.
    #[tokio::test]
    async fn abort_and_wait_with_nothing_running_completes() -> Result<(), error::Error> {
        let tasks = ShellTasks::default();
        assert_eq!(tasks.spawn(async { 7 }).await?, 7);
        tasks.abort_and_wait().await;
        Ok(())
    }
}
