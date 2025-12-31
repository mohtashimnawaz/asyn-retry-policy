use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

#[asyn_retry_policy::retry(predicate = |e: &String| e == "tmp")]
async fn macro_predicate_with_closure(tries: Arc<AtomicU8>) -> Result<u8, String> {
    let prev = tries.fetch_add(1, Ordering::SeqCst);
    if prev < 2 { Err(String::from("tmp")) } else { Ok(11u8) }
}

#[tokio::test]
async fn test_macro_predicate_closure_works() {
    let tries = Arc::new(AtomicU8::new(0));
    let res = macro_predicate_with_closure(tries.clone()).await;
    assert_eq!(res.unwrap(), 11u8);
    assert_eq!(tries.load(Ordering::SeqCst), 3);
}
