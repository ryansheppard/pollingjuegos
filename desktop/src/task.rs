use iced::Task;
use iced::futures::channel::oneshot;

/// Runs blocking work on its own thread and resolves when it finishes, so
/// iced's executor never blocks and no async runtime is needed.
pub fn blocking<T, M>(
    work: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
    to_message: impl FnOnce(Result<T, String>) -> M + Send + 'static,
) -> Task<M>
where
    T: Send + 'static,
    M: Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        // `{:#}` keeps anyhow's context chain, e.g. "exchanging login code: 400".
        let _ = tx.send(work().map_err(|e| format!("{e:#}")));
    });
    Task::perform(
        async move {
            rx.await
                .unwrap_or_else(|_| Err("background thread panicked".to_owned()))
        },
        to_message,
    )
}

/// Runs both closures at once on the current worker thread plus one more, for
/// requests that don't depend on each other.
pub fn both<A: Send, B>(a: impl FnOnce() -> A + Send, b: impl FnOnce() -> B) -> (A, B) {
    std::thread::scope(|s| {
        let a = s.spawn(a);
        let b = b();
        let a = a
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
        (a, b)
    })
}
