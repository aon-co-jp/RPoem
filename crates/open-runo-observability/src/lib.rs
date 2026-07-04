//! `open-runo-observability`: standardizes tracing/metrics setup across all
//! open-runo services. OpenTelemetry/Prometheus/Grafana export is planned
//! (see README section 9); this crate currently wires up structured
//! console logging via `tracing-subscriber` and a minimal in-process
//! counter registry for tests and local development.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::collections::HashMap;

/// Initialize a JSON-structured `tracing` subscriber reading its level
/// from `log_level` (e.g. `"info"`, `"debug"`). Safe to call once per
/// process at startup.
pub fn init_tracing(log_level: &str) {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(log_level.to_string()))
        .json()
        .try_init();
}

/// A minimal monotonic counter registry, useful for unit tests and local
/// dashboards before a full Prometheus exporter is wired in.
#[derive(Debug, Default)]
pub struct Counters {
    values: Mutex<HashMap<String, AtomicU64>>,
}

impl Counters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment(&self, name: &str) {
        // A poisoned lock still holds a valid (if possibly inconsistent)
        // map; recovering it is preferable to panicking a whole service
        // over a metrics counter.
        let mut values = self.values.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        values
            .entry(name.to_string())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn get(&self, name: &str) -> u64 {
        let values = self.values.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        values.get(name).map(|c| c.load(Ordering::Relaxed)).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_increment_independently() {
        let counters = Counters::new();
        counters.increment("requests_total");
        counters.increment("requests_total");
        counters.increment("errors_total");

        assert_eq!(counters.get("requests_total"), 2);
        assert_eq!(counters.get("errors_total"), 1);
        assert_eq!(counters.get("never_incremented"), 0);
    }

    #[test]
    fn init_tracing_is_idempotent() {
        // Calling twice must not panic (try_init swallows the second error).
        init_tracing("info");
        init_tracing("debug");
    }
}
