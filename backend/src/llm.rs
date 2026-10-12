//! OpenAI-compatible LLM client (vLLM on AMD by default).

use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::Config;
use crate::cost::CostModel;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LlmUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Clone)]
pub struct LlmClient {
    http: reqwest::Client,
    base: String,
    model: String,
    key: String,
    pub cost: CostModel,
}

impl LlmClient {
    pub fn new(cfg: &Config, benchmark: Option<&Value>) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(cfg.llm_timeout_secs))
                .build()
                .expect("http client"),
            base: cfg.llm_base_url.trim_end_matches('/').to_string(),
            model: cfg.llm_model.clone(),
            key: cfg.llm_api_key.clone(),
            cost: CostModel::new(&cfg.gpu_name, cfg.gpu_hourly_usd, benchmark),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub fn usd(&self, usage: &LlmUsage) -> f64 {
        self.cost.usd(usage.prompt_tokens, usage.completion_tokens)
    }

    /// Chat completion; returns (content, usage).
    pub async fn chat(&self, system: &str, user: &str, max_tokens: u32, temperature: f32) -> Result<(String, LlmUsage)> {
        let body = json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
            "temperature": temperature,
            "max_tokens": max_tokens,
        });
        let resp = self
            .http
            .post(format!("{}/chat/completions", self.base))
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await
            .context("calling model endpoint")?;
        let status = resp.status();
        let raw = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("model endpoint {}: {}", status, truncate(&raw, 300)));
        }
        let v: Value = serde_json::from_str(&raw).with_context(|| format!("bad model response: {}", truncate(&raw, 300)))?;
        let content = v["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| anyhow!("missing choices[0].message.content"))?
            .to_string();
        let usage = LlmUsage {
            prompt_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
            completion_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0),
            total_tokens: v["usage"]["total_tokens"].as_u64().unwrap_or(0),
        };
        Ok((content, usage))
    }

    /// Lightweight reachability probe for /api/health.
    pub async fn models(&self) -> Result<Value> {
        let http = reqwest::Client::builder().timeout(Duration::from_secs(4)).build()?;
        let resp = http
            .get(format!("{}/models", self.base))
            .bearer_auth(&self.key)
            .send()
            .await?;
        let v: Value = resp.json().await?;
        Ok(v)
    }
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n { s.to_string() } else { format!("{}…", &s[..n]) }
}

/// Pull the first JSON object out of a raw model response (handles code fences).
pub fn extract_json(raw: &str) -> Result<Value> {
    let start = raw.find('{').ok_or_else(|| anyhow!("no JSON object in response"))?;
    let end = raw.rfind('}').ok_or_else(|| anyhow!("no JSON object in response"))?;
    if end <= start {
        return Err(anyhow!("malformed JSON bounds"));
    }
    serde_json::from_str(&raw[start..=end]).context("parsing model JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_json_from_fences() {
        let raw = "```json\n{\"headline\": \"x\", \"n\": 3}\n```";
        let v = extract_json(raw).unwrap();
        assert_eq!(v["n"], 3);
    }
}
