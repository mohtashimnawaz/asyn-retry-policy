use asyn_retry_policy::RetryPolicy;
use std::sync::{Arc, atomic::{AtomicU8, Ordering}};

#[tokio::main]
async fn main() {
    let mut policy = RetryPolicy::default();
    policy.attempts = 4;
    policy.jitter = false;

    let tries = Arc::new(AtomicU8::new(0));
    let res = policy.retry(
        {
            let tries = tries.clone();
            move || {
                let tries = tries.clone();
                async move {
                    let prev = tries.fetch_add(1, Ordering::SeqCst);
                    if prev < 2 { Err::<u8, _>("temp") } else { Ok(()) }
                }
            }
        },
        |_| true,
    ).await;

    println!("Result: {:?}", res);
}
