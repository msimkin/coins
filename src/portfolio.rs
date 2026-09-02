//! Holdings from the config and from on-chain balances, added up.

use crate::data::Row;

#[derive(Debug, Clone)]
pub struct Holding {
    pub ticker: String,
    /// Palette slot, shared with the table and the chart.
    pub color: usize,
    pub value: f64,
}

/// One place holdings come from — a tracked address, or the config's own
/// `[holdings]`. Each gets its own group of rows rather than being smeared
/// across the coin rows as extra columns.
#[derive(Debug, Clone)]
pub struct HoldingSource {
    pub label: String,
    /// `None` for amounts typed into the config rather than read from a chain.
    pub address: Option<String>,
    /// What it holds: (coin id, amount), largest value first.
    pub coins: Vec<(String, f64)>,
}

#[derive(Debug, Clone)]
pub struct Portfolio {
    pub holdings: Vec<Holding>,
    pub total: f64,
    /// Where these numbers came from, each valued on its own.
    pub sources: Vec<HoldingSource>,
}

/// Builds the portfolio block, or `None` when nothing is held.
pub fn build(rows: &[Row], sources: Vec<HoldingSource>) -> Option<Portfolio> {
    let mut holdings: Vec<Holding> = Vec::new();
    let mut total = 0.0;
    // Value at the start of the range, for the change figure.
    let mut then_total = 0.0;
    let mut then_known = true;

    for row in rows {
        if row.amount <= 0.0 {
            continue;
        }
        let price = row.market.current_price.unwrap_or(0.0);
        let value = row.amount * price;
        total += value;
        match row.series.as_deref().and_then(first_price) {
            Some(p) => then_total += row.amount * p,
            None => then_known = false,
        }
        holdings.push(Holding {
            ticker: row.market.ticker(),
            color: row.color,
            value,
        });
    }
    if holdings.is_empty() {
        return None;
    }
    holdings.sort_by(|a, b| b.value.total_cmp(&a.value));
    let _ = (then_total, then_known);
    Some(Portfolio { holdings, total, sources })
}


fn first_price(series: &[(i64, f64)]) -> Option<f64> {
    series.iter().find(|(_, v)| v.is_finite() && *v > 0.0).map(|(_, v)| *v)
}

/// Percentages that add to exactly 100, formatted without the sign.
///
/// Rounding each share on its own does not do that: 0.9834, 0.0127 and 0.0039
/// become 98%, 1% and 0.4%, which total 99.4% and read as a bug. So the
/// precision is chosen as the coarsest at which nothing rounds away to zero
/// *and* the parts already total 100; failing that, the largest-remainder
/// method nudges whichever parts were rounded down hardest until they do.
pub fn percentages(shares: &[f64]) -> Vec<String> {
    if shares.is_empty() {
        return Vec::new();
    }
    let sum: f64 = shares.iter().sum();
    if sum <= 0.0 {
        return shares.iter().map(|_| "0".to_string()).collect();
    }
    let pct: Vec<f64> = shares.iter().map(|s| s / sum * 100.0).collect();

    // A precision where plain rounding already behaves needs no correction.
    let mut fallback = 2usize;
    for decimals in 0..=2usize {
        let scale = 10f64.powi(decimals as i32);
        let rounded: Vec<f64> = pct.iter().map(|p| (p * scale).round() / scale).collect();
        let any_zero = rounded
            .iter()
            .zip(&pct)
            .any(|(r, p)| *r == 0.0 && *p > 0.0);
        if any_zero {
            continue;
        }
        fallback = fallback.min(decimals);
        if (rounded.iter().sum::<f64>() - 100.0).abs() < 1e-9 {
            return rounded.iter().map(|r| format!("{r:.decimals$}")).collect();
        }
    }

    // Largest remainder: hand the shortfall to the parts with the biggest
    // fractional loss, so the visible total is exact.
    let decimals = fallback;
    let scale = 10f64.powi(decimals as i32);
    let target = (100.0 * scale).round() as i64;
    let mut units: Vec<i64> = pct.iter().map(|p| (p * scale).floor() as i64).collect();
    let mut order: Vec<usize> = (0..pct.len()).collect();
    order.sort_by(|a, b| {
        let rem = |i: usize| pct[i] * scale - (pct[i] * scale).floor();
        rem(*b).total_cmp(&rem(*a))
    });
    let mut deficit = target - units.iter().sum::<i64>();
    for &i in order.iter().cycle().take(deficit.unsigned_abs() as usize * 2) {
        if deficit == 0 {
            break;
        }
        if deficit > 0 {
            units[i] += 1;
            deficit -= 1;
        } else if units[i] > 0 {
            units[i] -= 1;
            deficit += 1;
        }
    }
    units
        .iter()
        .map(|u| format!("{:.decimals$}", *u as f64 / scale))
        .collect()
}

/// Share of the total, as a fraction, for the allocation bar.
pub fn shares(p: &Portfolio) -> Vec<(String, usize, f64)> {
    if p.total <= 0.0 {
        return Vec::new();
    }
    p.holdings
        .iter()
        .map(|h| (h.ticker.clone(), h.color, h.value / p.total))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(parts: &[String]) -> f64 {
        parts.iter().map(|p| p.parse::<f64>().unwrap()).sum()
    }

    #[test]
    fn percentages_always_add_to_a_hundred() {
        // The real portfolio that exposed this: 98% + 1% + 0.4% = 99.4%.
        let live = percentages(&[0.9834, 0.0127, 0.0039]);
        assert_eq!(live, vec!["98.3", "1.3", "0.4"]);
        assert!((total(&live) - 100.0).abs() < 1e-9);

        // Thirds cannot be written exactly at any precision, so the
        // largest-remainder correction has to carry them.
        let thirds = percentages(&[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0]);
        assert!((total(&thirds) - 100.0).abs() < 1e-9, "{thirds:?}");

        for shares in [
            vec![0.5, 0.5],
            vec![1.0],
            vec![0.7, 0.2, 0.1],
            vec![0.999, 0.001],
            vec![0.4, 0.3, 0.2, 0.07, 0.02, 0.01],
            vec![0.9834, 0.0127, 0.0039],
        ] {
            let parts = percentages(&shares);
            assert_eq!(parts.len(), shares.len());
            assert!(
                (total(&parts) - 100.0).abs() < 1e-9,
                "{shares:?} -> {parts:?} totals {}",
                total(&parts)
            );
        }
    }

    #[test]
    fn a_real_holding_never_shows_as_zero() {
        let parts = percentages(&[0.9995, 0.0005]);
        assert!(
            parts.last().unwrap().parse::<f64>().unwrap() > 0.0,
            "{parts:?}"
        );
    }

    #[test]
    fn whole_numbers_stay_whole_when_they_can() {
        assert_eq!(percentages(&[0.5, 0.5]), vec!["50", "50"]);
        assert_eq!(percentages(&[0.7, 0.2, 0.1]), vec!["70", "20", "10"]);
    }
}
