use asyn_retry_policy::retry;
use std::sync::{Arc, atomic::{AtomicU8, Ordering}};

#[retry(attempts = 3, predicate = |e: &str| e == "tmp")]
async fn do_work(tries: Arc<AtomicU8>) -> Result<u8, &'static str> {
    let prev = tries.fetch_add(1, Ordering::SeqCst);
    if prev < 2 { Err("tmp") } else { Ok(13u8) }
}

#[tokio::main]
async fn main() {
    let tries = Arc::new(AtomicU8::new(0));
    let res = do_work(tries.clone()).await;
    println!("res={:?}", res);
}
