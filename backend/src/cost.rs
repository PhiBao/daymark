//! Inference cost model: turns token usage into dollars using AMD GPU economics.
//!
//! Disclosed formula: cost_per_token = gpu_hourly_usd / 3600 / measured_tokens_per_sec.
//! When a measured benchmark exists (engine/artifacts/benchmark.json) it is used;
//! otherwise a conservative default is flagged as `estimated` in the API output.

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct CostModel {
    pub gpu_name: String,
    pub gpu_hourly_usd: f64,
    pub tokens_per_sec: f64,
    pub source: String, // "measured" | "estimated"
    pub usd_per_1m_tokens: f64,
}

impl CostModel {
    pub fn new(gpu_name: &str, gpu_hourly_usd: f64, benchmark: Option<&Value>) -> Self {
        let measured = benchmark
            .and_then(|b| b.get("decode_tokens_per_sec"))
            .and_then(|v| v.as_f64())
            .filter(|v| *v > 1.0);
        let (tps, source) = match measured {
            Some(t) => (t, "measured"),
            None => (45.0, "estimated"),
        };
        Self {
            gpu_name: gpu_name.to_string(),
            gpu_hourly_usd,
            tokens_per_sec: tps,
            source: source.to_string(),
            usd_per_1m_tokens: gpu_hourly_usd / 3600.0 * 1_000_000.0 / tps,
        }
    }

    pub fn usd(&self, prompt_tokens: u64, completion_tokens: u64) -> f64 {
        (prompt_tokens + completion_tokens) as f64 * self.usd_per_1m_tokens / 1_000_000.0
    }
}
