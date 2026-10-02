//! Bounded concurrent map for usage fan-out.

use std::future::Future;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Default parallel app fetches for `scout usage` (named budget).
pub const DEFAULT_USAGE_CONCURRENCY: usize = 4;

/// Hard ceiling so a typo cannot stampede the Scout API.
pub const MAX_USAGE_CONCURRENCY: usize = 32;

/// Resolve `--concurrency`: default 4, reject 0, clamp to [1, 32].
pub fn resolve_concurrency(requested: Option<u32>) -> Result<usize, String> {
    let value = requested.unwrap_or(DEFAULT_USAGE_CONCURRENCY as u32) as usize;
    if value == 0 {
        return Err("--concurrency must be at least 1".to_string());
    }
    Ok(value.min(MAX_USAGE_CONCURRENCY))
}

/// Run `f` over `items` with at most `limit` tasks in flight; preserve order.
pub async fn map_concurrent<T, R, F, Fut>(items: Vec<T>, limit: usize, f: F) -> Vec<R>
where
    T: Send + 'static,
    R: Send + 'static,
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
{
    let limit = limit.max(1);
    let semaphore = Arc::new(Semaphore::new(limit));
    let f = Arc::new(f);
    let mut handles = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        let semaphore = Arc::clone(&semaphore);
        let f = Arc::clone(&f);
        handles.push(tokio::spawn(async move {
            let _permit = semaphore
                .acquire()
                .await
                .expect("usage concurrency semaphore stays open");
            (index, f(item).await)
        }));
    }
    let mut indexed = Vec::with_capacity(handles.len());
    for handle in handles {
        indexed.push(handle.await.expect("usage fetch task"));
    }
    indexed.sort_by_key(|(index, _)| *index);
    indexed.into_iter().map(|(_, value)| value).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn resolve_defaults_and_clamps() {
        assert_eq!(
            resolve_concurrency(None).unwrap(),
            DEFAULT_USAGE_CONCURRENCY
        );
        assert_eq!(resolve_concurrency(Some(1)).unwrap(), 1);
        assert_eq!(
            resolve_concurrency(Some(100)).unwrap(),
            MAX_USAGE_CONCURRENCY
        );
        assert!(resolve_concurrency(Some(0)).is_err());
    }

    #[tokio::test]
    async fn map_concurrent_respects_cap() {
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let items: Vec<usize> = (0..8).collect();
        let in_flight_task = Arc::clone(&in_flight);
        let peak_task = Arc::clone(&peak);
        let results = map_concurrent(items, 2, move |n| {
            let in_flight = Arc::clone(&in_flight_task);
            let peak = Arc::clone(&peak_task);
            async move {
                let current = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                in_flight.fetch_sub(1, Ordering::SeqCst);
                n * 10
            }
        })
        .await;
        assert_eq!(results, vec![0, 10, 20, 30, 40, 50, 60, 70]);
        assert!(peak.load(Ordering::SeqCst) <= 2);
        assert!(peak.load(Ordering::SeqCst) >= 1);
    }
}
