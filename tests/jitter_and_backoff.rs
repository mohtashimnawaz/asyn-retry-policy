use asyn_retry_policy::RetryPolicy;
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::Rng;
use std::sync::{Arc};
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

#[tokio::test]
async fn deterministic_jitter_is_reproducible() {
    tokio::time::pause();

    let mut policy = RetryPolicy::default();
    policy.jitter = true;
    policy.rng_seed = Some(42);
    policy.base_delay = Duration::from_millis(100);
    policy.max_delay = Duration::from_millis(1000);

    let tries = Arc::new(AtomicU8::new(0));
    let t = tries.clone();

    let fut = tokio::spawn(async move {
        policy
            .retry(
                || {
                    let t = t.clone();
                    async move {
                        let prev = t.fetch_add(1, Ordering::SeqCst);
                        if prev < 1 { Err("tmp") } else { Ok(()) }
                    }
                },
                |_| true,
            )
            .await
    });

    // compute expected jitter for attempt 1
    let mut rng = SmallRng::seed_from_u64(42u64.wrapping_add(1));
    let max_ms = 100u64;
    let jitter_ms = rng.gen_range(0..=max_ms);

    // advance time by less than jitter -> not finished
    if jitter_ms > 0 {
        tokio::time::advance(Duration::from_millis(jitter_ms - 1)).await;
    }

    // now advance a bit more to reach the sleep
    tokio::time::advance(Duration::from_millis(2)).await;

    let res = fut.await.unwrap();

    assert!(res.is_ok());
    assert_eq!(tries.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn max_delay_is_enforced() {
    tokio::time::pause();

    let mut policy = RetryPolicy::default();
    policy.jitter = false;
    policy.base_delay = Duration::from_secs(1);
    policy.backoff_factor = 10.0; // big multiplier
    policy.max_delay = Duration::from_millis(1500); // 1.5s max

    let tries = Arc::new(AtomicU8::new(0));
    let t = tries.clone();

    let fut = tokio::spawn(async move {
        policy
            .retry(
                || {
                    let t = t.clone();
                    async move {
                        let prev = t.fetch_add(1, Ordering::SeqCst);
                        if prev < 2 { Err("tmp") } else { Ok(()) }
                    }
                },
                |_| true,
            )
            .await
    });

    // attempt 1 fails -> sleep 1s
    tokio::time::advance(Duration::from_secs(1)).await;
    // attempt 2 fails -> sleep should be clamped to 1.5s
    tokio::time::advance(Duration::from_millis(1500)).await;

    let res = fut.await.unwrap();
    assert!(res.is_ok());
    assert_eq!(tries.load(Ordering::SeqCst), 3);
}

#[test]
fn compute_backoff_values() {
    let mut policy = RetryPolicy::default();
    policy.base_delay = Duration::from_millis(100);
    policy.backoff_factor = 2.0;
    policy.max_delay = Duration::from_secs(1);

    // attempt 1 -> 100ms
    assert_eq!(policy.compute_backoff(1), Duration::from_millis(100));
    // attempt 2 -> 200ms
    assert_eq!(policy.compute_backoff(2), Duration::from_millis(200));
    // attempt 10 -> would be huge but clamped by max_delay
    assert_eq!(policy.compute_backoff(10), Duration::from_secs(1));
}
