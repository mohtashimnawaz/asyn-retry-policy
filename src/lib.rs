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
//!     let mut tries = 0;
//!     let res = policy.retry(|| async {
//!         tries += 1;
//!         if tries < 3 { Err::<(), _>("fail") } else { Ok(()) }
//!     }, |_| true).await;
//!     assert!(res.is_ok());
//! }
//! ```

use rand::Rng;
use std::time::Duration;

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
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_factor: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
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
                    let exp = self.backoff_factor.powi((attempt - 1) as i32);
                    let mut delay = self
                        .base_delay
                        .mul_f64(exp)
                        .min(self.max_delay);

                    // Apply jitter
                    if self.jitter {
                        let max_ms = delay.as_millis().max(1) as u64;
                        let jitter_ms = rand::thread_rng().gen_range(0..=max_ms);
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

    #[tokio::test]
    async fn retries_and_succeeds() {
        let policy = RetryPolicy::default();
        let mut tries = 0u8;
        let res = policy
            .retry(
                || {
                    async {
                        tries += 1;
                        if tries < 3 {
                            Err("temporary")
                        } else {
                            Ok(42u8)
                        }
                    }
                },
                |_| true,
            )
            .await;
        assert_eq!(res.unwrap(), 42u8);
        assert_eq!(tries, 3);
    }

    #[tokio::test]
    async fn stops_on_non_retryable_error() {
        let policy = RetryPolicy::default();
        let mut tries = 0u8;
        let res = policy
            .retry(
                || {
                    async {
                        tries += 1;
                        Err::<u8, _>("fatal")
                    }
                },
                |_e| false,
            )
            .await;
        assert!(res.is_err());
        assert_eq!(tries, 1);
    }
}
