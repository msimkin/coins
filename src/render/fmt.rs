//! Number, money and time formatting. Columns are read by eye and compared down
//! the page, so a price column shares one decimal count — the largest any of its
//! rows needs, up to `max_decimals` — which keeps every decimal mark in line.

use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Local, TimeZone};

use crate::config::Range;

/// Symbols for the currencies people actually watch; anything else falls back
/// to its upper-case code, which is honest and still aligns.
pub fn currency_symbol(code: &str) -> String {
    match code {
        "usd" | "aud" | "cad" | "nzd" | "sgd" | "hkd" | "mxn" | "clp" | "ars" => "$".into(),
        "eur" => "€".into(),
        "gbp" => "£".into(),
        "jpy" | "cny" => "¥".into(),
        "chf" => "Fr ".into(),
        "dkk" | "sek" | "nok" | "isk" => "kr ".into(),
        "pln" => "zł ".into(),
        "inr" => "₹".into(),
        "krw" => "₩".into(),
        "rub" => "₽".into(),
        "try" => "₺".into(),
        "brl" => "R$".into(),
        "zar" => "R ".into(),
        "btc" => "₿".into(),
        "eth" => "Ξ".into(),
        "sats" => "sat ".into(),
        other => format!("{} ", other.to_ascii_uppercase()),
    }
}

/// The ceiling on price decimals, set once from the config at startup.
static MAX_DECIMALS: OnceLock<usize> = OnceLock::new();

/// Called once from `main`; later calls are ignored.
pub fn set_max_decimals(n: usize) {
    let _ = MAX_DECIMALS.set(n);
}

pub fn max_decimals() -> usize {
    MAX_DECIMALS.get().copied().unwrap_or(3)
}

/// Decimals this value would like, to carry two significant digits: two down to
/// 0.10, then one more per leading zero. Uncapped — [`price_decimals`] and
/// [`column_decimals`] apply the ceiling.
fn needed_decimals(v: f64) -> usize {
    let a = v.abs();
    if a >= 0.1 || a == 0.0 || !a.is_finite() {
        return 2;
    }
    (1 - a.log10().floor() as i32).max(2) as usize
}

/// Decimals for a single price shown on its own.
pub fn price_decimals(v: f64) -> usize {
    needed_decimals(v).clamp(2, max_decimals().max(2))
}

/// One decimal count for a whole column: the largest any of its prices needs.
///
/// Rows in a price column are read against each other, so they must share a
/// precision — a column mixing two, three and four decimals puts the decimal
/// mark in a different place on every row, which is what made this unreadable.
pub fn column_decimals<I: IntoIterator<Item = f64>>(prices: I) -> usize {
    let cap = max_decimals().max(2);
    prices
        .into_iter()
        .filter(|v| v.is_finite() && *v != 0.0)
        .map(needed_decimals)
        .max()
        .unwrap_or(2)
        .clamp(2, cap)
}

/// True when a price is too small to survive `decimals` — it would print as
/// zero, which is worse than an unhelpfully long number.
pub fn rounds_to_zero(v: f64, decimals: usize) -> bool {
    v != 0.0 && v.is_finite() && format!("{:.*}", decimals, v.abs()).trim_matches(['0', '.']).is_empty()
}

/// Decimals that suit the magnitude: none for thousands, more as it approaches
/// zero. Still the right rule for a **chart axis**, whose labels share one
/// precision and whose gutter should not carry cents on a four-figure price.
pub fn decimals_for(v: f64) -> usize {
    let a = v.abs();
    if a >= 1000.0 {
        0
    } else if a >= 100.0 {
        1
    } else if a >= 1.0 {
        2
    } else if a >= 0.01 {
        4
    } else if a >= 0.0001 {
        6
    } else if a > 0.0 {
        8
    } else {
        2
    }
}

pub fn money(v: f64, currency: &str) -> String {
    format!("{}{}", currency_symbol(currency), group(v, price_decimals(v)))
}

/// A money value formatted to a fixed number of decimals, for a column where
/// every row must share one precision.
pub fn money_with(v: f64, currency: &str, decimals: usize) -> String {
    format!("{}{}", currency_symbol(currency), group(v, decimals))
}

/// The digit-group separator, set once from the config at startup.
///
/// A space, not a comma: `$2,372` is two-and-a-bit to a reader who uses `,` as
/// the decimal mark, and prices are exactly where that ambiguity costs the most.
static THOUSANDS: OnceLock<String> = OnceLock::new();

/// Called once from `main`; later calls are ignored.
pub fn set_thousands(separator: &str) {
    let _ = THOUSANDS.set(separator.to_string());
}

fn thousands() -> &'static str {
    THOUSANDS.get().map(|s| s.as_str()).unwrap_or(" ")
}

/// Group-separated fixed-point, using the configured separator.
pub fn group(v: f64, decimals: usize) -> String {
    group_with(thousands(), v, decimals)
}

/// Group-separated fixed-point with an explicit separator. The decimal mark is
/// always `.`, so a separator of `.` would be ambiguous — the config rejects it
/// only in combination, and the choice is the reader's.
pub fn group_with(separator: &str, v: f64, decimals: usize) -> String {
    let neg = v < 0.0;
    let s = format!("{:.*}", decimals, v.abs());
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i.to_string(), format!(".{f}")),
        None => (s, String::new()),
    };
    let mut out = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push_str(separator);
        }
        out.push(c);
    }
    format!("{}{}{}", if neg { "-" } else { "" }, out, frac)
}

/// A held quantity, as short as it can be while still saying something.
///
/// This goes in a free-text column rather than one scanned vertically, so each
/// amount can carry its own precision: whole units above a thousand, and below
/// that the same two-significant-digit rule prices use, with trailing zeros
/// trimmed. `0.000431` stays `0.00043` rather than collapsing to `0.0004`.
pub fn amount(v: f64) -> String {
    if v.abs() >= 1000.0 {
        return group(v, 0);
    }
    let text = group(v, needed_decimals(v).min(8));
    if text.contains('.') {
        // Trailing zeros say nothing about a quantity.
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text
    }
}

/// Strips characters that occupy no width but count as one — zero-width
/// spaces, joiners, byte-order marks, control codes. CoinGecko ships them in
/// at least one coin name, and every column to the right of one silently
/// loses its alignment.
pub fn clean_text(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(
                    *c,
                    '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{feff}' | '\u{00ad}'
                )
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub const UP: char = '▲';
pub const DOWN: char = '▼';

/// `▲2.70%` / `▼0.93%`. The glyph carries the sign so colour never has to.
pub fn percent(v: f64) -> String {
    let arrow = if v > 0.0 {
        UP
    } else if v < 0.0 {
        DOWN
    } else {
        '·'
    };
    let a = v.abs();
    // One decimal: the second is noise on a 28% move, and it costs a character
    // in every change column. Whole numbers past 1000% so a memecoin's yearly
    // figure cannot widen the column on its own.
    let decimals = if a >= 1000.0 { 0 } else { 1 };
    format!("{arrow}{:.*}%", decimals, a)
}

/// An axis or header label for a point in time, at the resolution the range
/// deserves: clock time for a day, dates for weeks, months for a year.
pub fn time_label(ms: i64, range: Range) -> String {
    let dt = match Local.timestamp_millis_opt(ms) {
        chrono::LocalResult::Single(dt) => dt,
        _ => return String::new(),
    };
    use chrono::Datelike;
    use chrono::Timelike;
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let month = MONTHS[(dt.month0() as usize).min(11)];
    match range {
        Range::D1 => format!("{:02}:{:02}", dt.hour(), dt.minute()),
        Range::W1 | Range::M1 | Range::M3 | Range::M6 => format!("{} {}", dt.day(), month),
        Range::Y1 => format!("{} '{:02}", month, dt.year() % 100),
        Range::All => format!("{}", dt.year()),
    }
}

/// "14s", "3m", "2h", "4d" — a duration at one significant unit.
pub fn span(age: Duration) -> String {
    let s = age.as_secs();
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m", s / 60)
    } else if s < 86_400 {
        format!("{}h", s / 3600)
    } else {
        format!("{}d", s / 86_400)
    }
}

const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A one-line trend, bucket-averaged into `cells` columns of eight levels.
pub fn sparkline(values: &[f64], cells: usize) -> String {
    if values.len() < 2 || cells == 0 {
        return " ".repeat(cells);
    }
    let buckets = bucket_average(values, cells);
    let min = buckets.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = buckets.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = max - min;
    buckets
        .iter()
        .map(|v| {
            if span <= 0.0 {
                BLOCKS[3]
            } else {
                let t = ((v - min) / span * 7.0).round().clamp(0.0, 7.0) as usize;
                BLOCKS[t]
            }
        })
        .collect()
}

/// Averages `values` down to `n` buckets, so a 4000-point series and a
/// 24-point one both render at the same width without aliasing.
pub fn bucket_average(values: &[f64], n: usize) -> Vec<f64> {
    if n == 0 || values.is_empty() {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            let lo = i * values.len() / n;
            let hi = (((i + 1) * values.len()) / n).max(lo + 1).min(values.len());
            let slice = &values[lo..hi];
            slice.iter().sum::<f64>() / slice.len() as f64
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands() {
        assert_eq!(group(66492.0, 0), "66 492");
        assert_eq!(group(1234567.891, 2), "1 234 567.89");
        // Rust's float formatting rounds an exact half to even.
        assert_eq!(group(-2070.5, 0), "-2 070");
        assert_eq!(group(-2071.4, 0), "-2 071");
        assert_eq!(group(0.5, 2), "0.50");
    }

    #[test]
    fn a_value_asks_for_two_decimals_down_to_a_tenth() {
        assert_eq!(needed_decimals(2047.24), 2);
        assert_eq!(needed_decimals(84.6957), 2);
        assert_eq!(needed_decimals(0.4321), 2);
        assert_eq!(needed_decimals(0.1), 2);
        // Then one more per leading zero, to keep two significant digits.
        assert_eq!(needed_decimals(0.022_477_4), 3);
        assert_eq!(needed_decimals(0.004_4), 4);
        assert_eq!(needed_decimals(0.000_002_94), 7);
        assert_eq!(needed_decimals(0.0), 2);
    }

    #[test]
    fn a_column_shares_the_largest_count_it_needs() {
        // The live portfolio: ETH and SOL want two, STRK wants three, so all
        // three rows get three — that is what keeps them comparable.
        assert_eq!(column_decimals([2050.59, 84.64, 0.022_477_4]), 3);
        // Nothing small: everyone stays at two.
        assert_eq!(column_decimals([2050.59, 84.64]), 2);
        // The ceiling holds however cheap the coin.
        assert_eq!(column_decimals([2050.59, 0.000_002_94]), 3);
        assert_eq!(column_decimals([]), 2);
    }

    #[test]
    fn a_price_lost_to_the_ceiling_is_detectable() {
        assert!(rounds_to_zero(0.000_002_94, 3));
        assert!(!rounds_to_zero(0.022_477_4, 3));
        assert!(!rounds_to_zero(0.0, 3));
    }

    #[test]
    fn percents_lose_the_second_decimal() {
        assert_eq!(percent(28.1), "▲28.1%");
        assert_eq!(percent(-46.1), "▼46.1%");
        assert_eq!(percent(-3.370_93), "▼3.4%");
        assert_eq!(percent(2400.0), "▲2400%");
    }

    #[test]
    fn separators_are_configurable() {
        assert_eq!(group_with(" ", 76592.0, 0), "76 592");
        assert_eq!(group_with(",", 76592.0, 0), "76,592");
        assert_eq!(group_with(".", 76592.0, 0), "76.592");
        assert_eq!(group_with("", 76592.0, 0), "76592");
        assert_eq!(group_with(" ", 1234567.891, 2), "1 234 567.89");
        // A space default is what makes this unambiguous for a four-digit price.
        assert_eq!(group_with(" ", 2372.0, 0), "2 372");
    }

    #[test]
    fn precision_follows_magnitude() {
        assert_eq!(money(66492.0, "eur"), "€66 492.00");
        assert_eq!(money(2070.53, "usd"), "$2 070.53");
        assert_eq!(money(0.4321, "usd"), "$0.43");
        // Capped at three, so a cheap coin shown alone stops there (and an
        // exact half rounds to even, as everywhere else in Rust's formatting).
        assert_eq!(money(0.0525, "usd"), "$0.052");
        assert_eq!(money(0.000_018_2, "usd"), "$0.000");
    }

    #[test]
    fn percents_carry_a_glyph() {
        assert_eq!(percent(2.7), "▲2.7%");
        assert_eq!(percent(-0.93), "▼0.9%");
        assert_eq!(percent(0.0), "·0.0%");
    }

    #[test]
    fn invisible_characters_are_stripped() {
        assert_eq!(clean_text("\u{200b}\u{200b}Stable"), "Stable");
        assert_eq!(clean_text("Bitcoin"), "Bitcoin");
        assert_eq!(clean_text("a\u{feff}b\tc"), "abc");
        // The width of what we print must match what we measured.
        let cleaned = clean_text("\u{200b}USDC");
        assert_eq!(cleaned.chars().count(), 4);
    }

    #[test]
    fn spans_read_as_one_unit() {
        assert_eq!(span(Duration::from_secs(14)), "14s");
        assert_eq!(span(Duration::from_secs(200)), "3m");
        assert_eq!(span(Duration::from_secs(7300)), "2h");
    }

    #[test]
    fn amounts_are_short_but_never_nothing() {
        assert_eq!(amount(18.406231), "18.41");
        // Grouped with the configured separator, a space by default.
        assert_eq!(amount(12345.678), "12 346");
        assert_eq!(amount(7.284915), "7.28");
        assert_eq!(amount(0.5), "0.5");
        assert_eq!(amount(1.0), "1");
        // Two decimals would print a dust holding as nothing.
        assert_eq!(amount(0.000431), "0.00043");
        assert_eq!(amount(0.0003), "0.0003");
    }

    #[test]
    fn sparkline_has_one_cell_per_column() {
        let s = sparkline(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 8);
        assert_eq!(s.chars().count(), 8);
        assert_eq!(sparkline(&[], 4).chars().count(), 4);
    }

    #[test]
    fn buckets_average_down() {
        assert_eq!(bucket_average(&[1.0, 3.0, 5.0, 7.0], 2), vec![2.0, 6.0]);
    }
}
