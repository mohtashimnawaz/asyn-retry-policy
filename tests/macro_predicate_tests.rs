use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

fn is_retryable(err: &str) -> bool { err == "tmp" }
fn never_retry(_: &str) -> bool { false }

#[asyn_retry_policy::retry(attempts = 3, predicate = is_retryable)]
async fn macro_predicate_works(tries: Arc<AtomicU8>) -> Result<u8, &'static str> {
    let prev = tries.fetch_add(1, Ordering::SeqCst);
    if prev < 2 { Err("tmp") } else { Ok(5u8) }
}

#[tokio::test]
async fn test_macro_predicate_works() {
    let tries = Arc::new(AtomicU8::new(0));
    let res = macro_predicate_works(tries.clone()).await;
    assert_eq!(res.unwrap(), 5u8);
    assert_eq!(tries.load(Ordering::SeqCst), 3);
}

#[asyn_retry_policy::retry(predicate = never_retry)]
async fn macro_predicate_blocks_retry(tries: Arc<AtomicU8>) -> Result<u8, &'static str> {
    tries.fetch_add(1, Ordering::SeqCst);
    Err("fatal")
}

#[tokio::test]
async fn test_macro_predicate_blocks() {
    let tries = Arc::new(AtomicU8::new(0));
    let res = macro_predicate_blocks_retry(tries.clone()).await;
    assert!(res.is_err());
    assert_eq!(tries.load(Ordering::SeqCst), 1);
}

// Also verify using a string path is accepted
#[asyn_retry_policy::retry(predicate = "is_retryable")]
async fn macro_predicate_with_string_path(tries: Arc<AtomicU8>) -> Result<u8, &'static str> {
    let prev = tries.fetch_add(1, Ordering::SeqCst);
    if prev < 2 { Err("tmp") } else { Ok(6u8) }
}

#[tokio::test]
async fn test_macro_predicate_string_path() {
    let tries = Arc::new(AtomicU8::new(0));
    let res = macro_predicate_with_string_path(tries.clone()).await;
    assert_eq!(res.unwrap(), 6u8);
    assert_eq!(tries.load(Ordering::SeqCst), 3);
}
