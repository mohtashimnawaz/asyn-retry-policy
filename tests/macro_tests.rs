use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

// The attribute macro is exported by the crate root as `retry` but attributes are resolved by path,
// so we can use the fully-qualified path to the proc-macro crate: `asyn_retry_policy::retry`.

#[asyn_retry_policy::retry(3)]
async fn macro_retries_and_succeeds(tries: Arc<AtomicU8>) -> Result<u8, &'static str> {
    let prev = tries.fetch_add(1, Ordering::SeqCst);
    if prev < 2 {
        Err("temporary")
    } else {
        Ok(99u8)
    }
}

#[tokio::test]
async fn macro_test_retries_and_succeeds() {
    let tries = Arc::new(AtomicU8::new(0));
    let res = macro_retries_and_succeeds(tries.clone()).await;
    assert_eq!(res.unwrap(), 99u8);
    assert_eq!(tries.load(Ordering::SeqCst), 3);
}

#[asyn_retry_policy::retry]
async fn macro_defaults_to_three_attempts(tries: Arc<AtomicU8>) -> Result<u8, &'static str> {
    let prev = tries.fetch_add(1, Ordering::SeqCst);
    if prev < 2 { Err("nope") } else { Ok(7u8) }
}

#[tokio::test]
async fn macro_default_attempts() {
    let tries = Arc::new(AtomicU8::new(0));
    let res = macro_defaults_to_three_attempts(tries.clone()).await;
    assert_eq!(res.unwrap(), 7u8);
    assert_eq!(tries.load(Ordering::SeqCst), 3);
}
