use std::path::PathBuf;

/// Runtime configuration, all via environment with sane demo defaults.
#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    /// OpenAI-compatible endpoint (vLLM on AMD by default; any local server works for dev).
    pub llm_base_url: String,
    pub llm_model: String,
    pub llm_api_key: String,
    pub llm_timeout_secs: u64,
    /// AMD GPU price model used for the per-brief cost meter (disclosed in /api/meta).
    pub gpu_hourly_usd: f64,
    pub gpu_name: String,
}

impl Config {
    pub fn from_env() -> Self {
        let root = std::env::var("DAYMARK_ROOT").unwrap_or_else(|_| "..".into());
        let root = PathBuf::from(root);
        Self {
            port: std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8787),
            data_dir: std::env::var("DAYMARK_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| root.join("engine/artifacts")),
            state_dir: std::env::var("DAYMARK_STATE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| root.join("backend/state")),
            llm_base_url: std::env::var("DAYMARK_LLM_BASE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8000/v1".into()),
            llm_model: std::env::var("DAYMARK_LLM_MODEL").unwrap_or_else(|_| "gemma".into()),
            llm_api_key: std::env::var("DAYMARK_LLM_API_KEY").unwrap_or_else(|_| "daymark".into()),
            llm_timeout_secs: std::env::var("DAYMARK_LLM_TIMEOUT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120),
            gpu_hourly_usd: std::env::var("DAYMARK_GPU_HOURLY_USD")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.99),
            gpu_name: std::env::var("DAYMARK_GPU_NAME").unwrap_or_else(|_| "AMD Instinct (AMD Developer Cloud)".into()),
        }
    }
}
