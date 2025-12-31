# Changelog

All notable changes to this project will be documented in this file.

## Unreleased

---

## 0.1.0 - 2025-12-31

- Initial public API with `RetryPolicy`.
- `#[retry]` attribute proc-macro with options: `attempts`, `base_delay_ms`, `max_delay_ms`, `backoff_factor`, `jitter`, `rng_seed`, `predicate`.
- Deterministic jitter support for testing.
- Tests and examples.
