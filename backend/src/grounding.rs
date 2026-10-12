//! Number-grounding: the structural guarantee that the LLM never invents a figure.
//!
//! Generated copy is scanned for every numeric token; each one must be present
//! in the facts payload (with rounding tolerance) or the copy is rejected and
//! regenerated once, then replaced by a deterministic template.

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroundingReport {
    pub ok: bool,
    pub checked: usize,
    pub violations: Vec<String>,
    pub source: String,
}

impl GroundingReport {
    pub fn skipped(why: &str) -> Self {
        Self { ok: true, checked: 0, violations: vec![], source: why.into() }
    }
}

fn normalize_number(raw: &str) -> Option<f64> {
    // no lookaround in the regex crate: collapse thousands separators iteratively
    let thousands = Regex::new(r"(\d),(\d{3})").unwrap();
    let mut s = raw.trim().to_string();
    for _ in 0..3 {
        let next = thousands.replace_all(&s, "$1$2").to_string();
        if next == s {
            break;
        }
        s = next;
    }
    if s.contains(',') && !s.contains('.') {
        s = s.replace(',', ".");
    }
    s.trim_end_matches('%').parse::<f64>().ok()
}

/// Extract numeric tokens (optionally followed by `%`).
pub fn extract_numbers(text: &str) -> Vec<String> {
    let re = Regex::new(r"\d[\d,\.]*\s*%?").unwrap();
    re.find_iter(text)
        .map(|m| m.as_str().trim().trim_end_matches(['.', ',']).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Collect every numeric value inside a JSON document (recursively).
pub fn collect_facts(value: &serde_json::Value, out: &mut Vec<f64>) {
    match value {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                out.push(f);
            }
        }
        serde_json::Value::String(s) => {
            // numbers embedded in strings (dates, label fragments) are allowed too
            for tok in extract_numbers(s) {
                if let Some(f) = normalize_number(&tok) {
                    out.push(f);
                }
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|v| collect_facts(v, out)),
        serde_json::Value::Object(m) => m.values().for_each(|v| collect_facts(v, out)),
        _ => {}
    }
}

/// Verify every number in `text` against `allowed`. `extra` covers dates and
/// identifiers the copy may legitimately reference.
pub fn verify(text: &str, allowed: &[f64], extra: &[f64]) -> GroundingReport {
    let mut checked = 0usize;
    let mut violations = Vec::new();
    for tok in extract_numbers(text) {
        let Some(n) = normalize_number(&tok) else { continue };
        checked += 1;
        let ok = allowed
            .iter()
            .chain(extra.iter())
            .any(|f| (n - f).abs() <= 0.51 || (f.abs() > 1e-9 && (n - f).abs() / f.abs() <= 0.01));
        if !ok {
            violations.push(tok);
        }
    }
    GroundingReport {
        ok: violations.is_empty(),
        checked,
        violations,
        source: "grounding_checker".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_and_normalizes() {
        let nums = extract_numbers("Order 1,240 units (12.5%) at $0.30 off");
        assert!(nums.contains(&"1,240".to_string()));
        assert_eq!(normalize_number("1,240"), Some(1240.0));
        assert_eq!(normalize_number("12.5%"), Some(12.5));
    }

    #[test]
    fn accepts_grounded_copy() {
        let facts = json!({"total_units": 1240, "waste_avoided_usd": 186.5, "items": [12, 34]});
        let mut allowed = Vec::new();
        collect_facts(&facts, &mut allowed);
        let ok = verify("Place 1,240 units. This avoids $186.50 of waste.", &allowed, &[]);
        assert!(ok.ok, "violations: {:?}", ok.violations);
        assert_eq!(ok.checked, 2);
    }

    #[test]
    fn rejects_invented_numbers() {
        let facts = json!({"total_units": 1240});
        let mut allowed = Vec::new();
        collect_facts(&facts, &mut allowed);
        let bad = verify("Place 1,240 units and save $999.", &allowed, &[]);
        assert!(!bad.ok);
        assert_eq!(bad.violations, vec!["999".to_string()]);
    }

    #[test]
    fn extra_numbers_allowed() {
        let bad = verify("Published 2017-08-14 for store 2.", &[], &[2017.0, 8.0, 14.0, 2.0]);
        assert!(bad.ok);
    }
}
