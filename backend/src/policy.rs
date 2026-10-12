//! Decision policies: quantile handling, newsvendor ordering, markdown triggers.
//!
//! All product math lives here so the simulator, the brief and the tests share
//! exactly one implementation.

/// Piecewise-linear quantile estimator from a p10/p50/p90 band.
/// Extrapolates linearly beyond the band (slope of the adjacent segment).
pub fn quantile_from_band(p10: f64, p50: f64, p90: f64, s: f64) -> f64 {
    let (p10, p50, p90) = (p10.max(0.0), p50.max(0.0), p90.max(0.0));
    let q = if s <= 0.1 {
        let slope = (p50 - p10) / 0.4;
        p10 - (0.1 - s) * slope
    } else if s <= 0.5 {
        p10 + (s - 0.1) / 0.4 * (p50 - p10)
    } else if s <= 0.9 {
        p50 + (s - 0.5) / 0.4 * (p90 - p50)
    } else {
        let slope = (p90 - p50) / 0.4;
        p90 + (s - 0.9) * slope
    };
    q.max(0.0)
}

#[derive(Clone, Debug)]
pub struct OrderAdvice {
    pub target: f64,
    pub qty: i32,
    pub cover_days: f64,
    pub service_level: f64,
}

/// Newsvendor-style order: cover demand up to `service_level`, capped by what
/// can physically sell before the lot expires (optimistic p90 × shelf life),
/// minus what is already on hand.
pub fn order_advice(
    onhand: f64,
    p10: f64,
    p50: f64,
    p90: f64,
    service_level: f64,
    shelf_life_days: i32,
    avg_daily_sales: f64,
) -> OrderAdvice {
    let q = quantile_from_band(p10, p50, p90, service_level);
    let cap = (shelf_life_days.max(1) as f64) * p90.max(1.0);
    let target = q.min(cap);
    let need = (target - onhand).max(0.0);
    OrderAdvice {
        target,
        qty: need.ceil() as i32,
        cover_days: if avg_daily_sales > 0.05 { onhand / avg_daily_sales } else { f64::INFINITY },
        service_level,
    }
}

/// Markdown trigger by remaining shelf life (days). Returns the price cut.
pub fn markdown_cut(remaining_days: i64) -> Option<f64> {
    match remaining_days {
        d if d <= 1 => Some(0.50),
        2 => Some(0.30),
        _ => None,
    }
}

/// Demand multiplier from a markdown, scaled from the measured promo lift for
/// the family. `lift_pct` is the measured on-promotion lift; a 30% cut maps to
/// the full measured lift, deeper cuts scale linearly (declared assumption).
pub fn markdown_demand_boost(lift_pct: f64, cut: f64, lift_scale: f64) -> f64 {
    let base = (lift_pct / 100.0).max(0.0);
    1.0 + lift_scale * base * (cut / 0.30)
}

/// "Spreadsheet" baseline order: last same-weekday sales × 1.05.
pub fn spreadsheet_order(last_same_weekday: f64, onhand: f64) -> i32 {
    let target = last_same_weekday * 1.05;
    ((target - onhand).max(0.0)).ceil() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantile_interpolates_monotonically() {
        let (p10, p50, p90) = (8.0, 12.0, 20.0);
        let mut prev = -1.0;
        for s in [0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.95, 1.0] {
            let q = quantile_from_band(p10, p50, p90, s);
            assert!(q >= prev, "quantiles must be monotone in s ({s})");
            prev = q;
        }
        assert!((quantile_from_band(p10, p50, p90, 0.5) - 12.0).abs() < 1e-9);
        assert!((quantile_from_band(p10, p50, p90, 0.1) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn quantile_clamps_at_zero() {
        assert_eq!(quantile_from_band(0.0, 0.0, 0.0, 0.9), 0.0);
        assert!(quantile_from_band(0.0, 0.0, 2.0, 0.0) >= 0.0);
    }

    #[test]
    fn order_never_negative_and_respects_onhand() {
        let adv = order_advice(100.0, 5.0, 10.0, 20.0, 0.92, 5, 10.0);
        assert_eq!(adv.qty, 0);
        let adv2 = order_advice(0.0, 5.0, 10.0, 20.0, 0.92, 5, 10.0);
        assert!(adv2.qty > 10);
    }

    #[test]
    fn order_capped_by_shelf_life() {
        // p90=100/day but shelf life 1 day → cannot order more than ~100
        let adv = order_advice(0.0, 50.0, 80.0, 100.0, 0.99, 1, 80.0);
        assert!(adv.target <= 100.0 + 1e-9);
    }

    #[test]
    fn markdown_thresholds() {
        assert_eq!(markdown_cut(0), Some(0.50));
        assert_eq!(markdown_cut(1), Some(0.50));
        assert_eq!(markdown_cut(2), Some(0.30));
        assert_eq!(markdown_cut(3), None);
    }

    #[test]
    fn boost_scales_with_cut() {
        let b30 = markdown_demand_boost(20.0, 0.30, 1.0);
        let b50 = markdown_demand_boost(20.0, 0.50, 1.0);
        assert!((b30 - 1.2).abs() < 1e-9);
        assert!(b50 > b30);
    }
}
