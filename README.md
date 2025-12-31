# asyn-retry-policy 🕸️

A small, ergonomic crate that provides async retry behavior with exponential backoff and optional jitter.

Features
- Programmatic API: `RetryPolicy::retry(...)` for direct control
- Ergonomic macro: `#[retry]` or `#[retry(N)]` and named options (e.g., `attempts`, `base_delay_ms`, `max_delay_ms`, `backoff_factor`, `jitter`, `rng_seed`, `predicate`).

Quick examples

Programmatic example with a predicate (owned `String` error type):

```rust
use asyn_retry_policy::RetryPolicy;
use std::sync::{Arc, atomic::{AtomicU8, Ordering}};

fn is_retryable(e: &String) -> bool { e == "temporary" }

#[tokio::main]
async fn main() {
    let mut policy = RetryPolicy::default();
    policy.attempts = 5;
    policy.jitter = false;

    let tries = Arc::new(AtomicU8::new(0));
    let res = policy.retry(
        {
            let tries = tries.clone();
            move || {
                let tries = tries.clone();
                async move {
                    let prev = tries.fetch_add(1, Ordering::SeqCst);
                    if prev < 2 { Err::<u8, _>(String::from("temporary")) } else { Ok(()) }
                }
            }
        },
        is_retryable,
    ).await;
    assert!(res.is_ok());
}
```

Macro usage (predicate as a path):

```rust
use asyn_retry_policy::retry;
use std::sync::{Arc, atomic::{AtomicU8, Ordering}};

fn should_retry(e: &String) -> bool { e == "tmp" }

#[retry(attempts = 3, predicate = should_retry)]
async fn my_endpoint(tries: Arc<AtomicU8>) -> Result<u8, String> {
    let prev = tries.fetch_add(1, Ordering::SeqCst);
    if prev < 2 { Err(String::from("tmp")) } else { Ok(7u8) }
}
```

Notes
- Predicate signatures: the predicate receives `&E` (a reference to the error type). For example, if your operation returns `Result<T, String>`, implement the predicate as `fn pred(e: &String) -> bool`.
- Use `rng_seed` to make jitter deterministic for tests.

License: MIT
