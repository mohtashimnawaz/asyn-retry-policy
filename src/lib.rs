//! A small crate providing an async retry policy with exponential backoff and jitter.
//!
//! Example:
//!
//! ```no_run
//! use asyn_retry_policy::RetryPolicy;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() {
//!     let policy = RetryPolicy::default();
//!     let tries = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(0));
//!     let res = policy.retry(
//!         {
//!             let tries = tries.clone();
//!             move || {
//!                 let tries = tries.clone();
//!                 async move {
//!                     let prev = tries.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
//!                     if prev < 2 { Err::<(), _>("fail") } else { Ok(()) }
//!                 }
//!             }
//!         },
//!         |_| true,
//!     ).await;
//!     assert!(res.is_ok());
//! }
//! ```

use rand::Rng;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use std::time::Duration;

// Re-export the proc-macro so users can just write `#[retry]` or `#[retry(3)]` when depending on this crate
pub use asyn_retry_policy_macro::retry;

/// Retry policy configuration
#[derive(Clone, Debug)]
pub struct RetryPolicy {
    /// Maximum number of attempts (including the first try)
    pub attempts: usize,
    /// Base delay to use for backoff
    pub base_delay: Duration,
    /// Maximum delay between attempts
    pub max_delay: Duration,
    /// Multiplicative backoff factor
    pub backoff_factor: f64,
    /// Use random jitter between 0..delay
    pub jitter: bool,
    /// Optional RNG seed to allow deterministic jitter for testing
    pub rng_seed: Option<u64>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_factor: 2.0,
            jitter: true,
            rng_seed: None,
        }
    }
}

impl RetryPolicy {
    /// Compute the exponential backoff (without jitter) clamped by `max_delay`.
    pub fn compute_backoff(&self, attempt: usize) -> Duration {
        let exp = self.backoff_factor.powi((attempt - 1) as i32);
        self.base_delay.mul_f64(exp).min(self.max_delay)
    }

    /// Retry an asynchronous operation described by `f` with this policy.
    ///
    /// `f` must return a `Result<T, E>`. The `should_retry` predicate receives a reference to the error
    /// and returns whether the operation should be retried.
    pub async fn retry<Fut, T, E, F, P>(&self, mut f: F, mut should_retry: P) -> Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>> + Send,
        T: Send,
        E: Send,
        P: FnMut(&E) -> bool,
    {
        for attempt in 1..=self.attempts {
            match f().await {
                Ok(v) => return Ok(v),
                Err(e) if attempt < self.attempts && should_retry(&e) => {
                    // Calculate exponential backoff
                    let mut delay = self.compute_backoff(attempt);

                    // Apply jitter
                    if self.jitter {
                        let max_ms = delay.as_millis().max(1) as u64;
                        let jitter_ms = if let Some(seed) = self.rng_seed {
                            // deterministic per-attempt RNG to keep testability
                            let mut rng = SmallRng::seed_from_u64(seed.wrapping_add(attempt as u64));
                            rng.gen_range(0..=max_ms)
                        } else {
                            rand::thread_rng().gen_range(0..=max_ms)
                        };
                        delay = Duration::from_millis(jitter_ms);
                    }

                    tokio::time::sleep(delay).await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!("loop returns or errors")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};

    #[tokio::test]
    async fn retries_and_succeeds() {
        let policy = RetryPolicy::default();
        let tries = Arc::new(AtomicU8::new(0));
        let res = policy
            .retry(
                {
                    let tries = tries.clone();
                    move || {
                        let tries = tries.clone();
                        async move {
                            let prev = tries.fetch_add(1, Ordering::SeqCst);
                            if prev < 2 {
                                Err("temporary")
                            } else {
                                Ok(42u8)
                            }
                        }
                    }
                },
                |_| true,
            )
            .await;
        assert_eq!(res.unwrap(), 42u8);
        assert_eq!(tries.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn stops_on_non_retryable_error() {
        let policy = RetryPolicy::default();
        let tries = Arc::new(AtomicU8::new(0));
        let res = policy
            .retry(
                {
                    let tries = tries.clone();
                    move || {
                        let tries = tries.clone();
                        async move {
                            tries.fetch_add(1, Ordering::SeqCst);
                            Err::<u8, _>("fatal")
                        }
                    }
                },
                |_e| false,
            )
            .await;
        assert!(res.is_err());
        assert_eq!(tries.load(Ordering::SeqCst), 1);
    }
}
