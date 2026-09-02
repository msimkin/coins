//! Screen composition: header, table, chart, portfolio.

pub mod braille;
pub mod fmt;
pub mod table;
pub mod theme;

use crate::config::Config;
use crate::data::{ChartPlan, Row, Snapshot, Status};
use crate::portfolio::{self, Portfolio};
use braille::{Plot, PlotSeries};
use theme::{SLine, Theme, vis_width};

const INDENT: &str = "  ";
/// Past this width the eye has to travel too far between columns.
const MAX_WIDTH: usize = 128;

/// The whole screen, as lines ready to print.
pub fn screen(snap: &Snapshot, cfg: &Config, theme: &Theme, term_width: usize) -> Vec<String> {
    let width = term_width.min(MAX_WIDTH).saturating_sub(INDENT.len() * 2);

    // The body is built first so the header and its rule can be sized to the
    // content. Stretching them to the terminal instead leaves a rule hanging
    // far past the last column, which reads as something missing.
    let mut body: Vec<String> = Vec::new();
    let mut showed_addresses = false;
    let chart = chart(snap, cfg, theme, width);
    if snap.show_table {
        let rendered = table::table(snap, cfg, theme, width);
        showed_addresses = rendered.with_addresses;
        body.extend(rendered.lines);
    } else if snap.plan == ChartPlan::Single {
        if let Some(row) = snap.rows.first() {
            body.extend(hero(row, snap, theme));
        }
    }
    if !chart.is_empty() {
        // Only separate the chart from something above it.
        if !body.is_empty() {
            body.push(String::new());
        }
        body.extend(chart);
    }

    // Sized to the content, not the terminal. When the table cannot be made to
    // fit — five change columns have a floor — a rule that matches it still
    // reads as deliberate, where a short one reads as something broken.
    let content = body
        .iter()
        .map(|l| vis_width(&braille::strip_ansi(l)))
        .max()
        .unwrap_or(width)
        .clamp(36, MAX_WIDTH - INDENT.len() * 2);

    if let Some(p) = &snap.portfolio {
        // The address groups live inside the table, so they share its column
        // widths. Only the summary is left to add here — and only when it says
        // something those groups do not: more than one source to add up, or
        // more than one coin to split.
        // Never without the totals row it belongs to — a terminal too narrow
        // for those columns would otherwise leave it floating under the coins
        // — but set off from it, since it is a shape and not a row of figures.
        if snap.show_table && snap.show_addresses && showed_addresses {
            let bar = portfolio_block(p, snap, theme, content);
            if !bar.is_empty() {
                body.push(String::new());
                body.extend(bar);
            }
        }
    }

    let mut out = vec![header(snap, theme, content)];
    let rule = "─".repeat(content);
    out.push(indent(&theme.paint(&rule, theme.axis())));
    out.extend(body.into_iter().map(|l| indent(&l)));
    for w in &snap.warnings {
        out.push(indent(&theme.dim(&format!("! {w}"))));
    }
    out
}

fn indent(line: &str) -> String {
    if line.is_empty() {
        String::new()
    } else {
        format!("{INDENT}{line}")
    }
}

/// The line above the rule: nothing but how old the numbers are, right-aligned.
/// A title repeating the currency and the period earned no space, since the
/// currency is on every price and the period heads the plot column.
fn header(snap: &Snapshot, theme: &Theme, width: usize) -> String {
    let status = status_text(snap);
    let mut line = SLine::new();
    line.pad_to(width.saturating_sub(vis_width(&status)));
    line.styled(&status, theme.dim(&status));
    indent(&line.finish())
}

fn status_text(snap: &Snapshot) -> String {
    let age = fmt::span(snap.age);
    match snap.status {
        Status::Fresh => format!("updated {age} ago"),
        Status::Warming => format!("updated {age} ago · refreshing"),
        Status::Offline => format!("offline · prices {age} old"),
        Status::RateLimited => format!("rate-limited · prices {age} old"),
    }
}

/// The single-coin view's headline: the number, its change, and the period's
/// extremes — which is the whole story for one coin.
/// One coin's identity and figures, on the line below the rule: the coloured
/// chip that names it in every other view, then the name, then the numbers —
/// left-aligned at the margin.
fn hero(row: &Row, snap: &Snapshot, theme: &Theme) -> Vec<String> {
    let mut line = SLine::new();
    let chip = format!("● {}", row.market.ticker());
    line.styled(&chip, theme.paint(&chip, theme.series(row.color)));
    let name = format!(" ({})", fmt::clean_text(&row.market.name));
    line.styled(&name, theme.dim(&name));
    line.spaces(2);
    let price = match row.market.current_price {
        Some(p) => fmt::money(p, &snap.currency),
        None => "·".into(),
    };
    line.styled(&price, theme.bold(&price));
    line.spaces(3);

    let range_change = row
        .series
        .as_deref()
        .and_then(change_over)
        .or_else(|| row.market.change("24h"));
    if let Some(v) = range_change {
        let text = fmt::percent(v);
        line.styled(&text, theme.delta(&text, v));
        let suffix = format!(" ({})", snap.range.short().to_ascii_lowercase());
        line.styled(&suffix, theme.dim(&suffix));
    }

    let mut out = vec![line.finish().trim_end().to_string()];
    // A price curve is a thing you can show someone. The holding follows the
    // same rule as every other holdings figure — off unless asked for — so
    // `coins plot eth` says nothing about what you own.
    if row.amount > 0.0 && snap.show_addresses {
        let value = row.amount * row.market.current_price.unwrap_or(0.0);
        let text = format!(
            "holding {} {} · {}",
            fmt::group(row.amount, 4),
            row.market.ticker(),
            fmt::money_with(value, &snap.currency, 0)
        );
        out.push(theme.dim(&text));
    }
    out
}

fn change_over(series: &[(i64, f64)]) -> Option<f64> {
    let first = series.iter().find(|(_, v)| v.is_finite() && *v > 0.0)?.1;
    let last = series.iter().rev().find(|(_, v)| v.is_finite())?.1;
    Some((last - first) / first * 100.0)
}

// -------------------------------------------------------------------- chart ---

fn chart(snap: &Snapshot, cfg: &Config, theme: &Theme, width: usize) -> Vec<String> {
    match snap.plan {
        ChartPlan::Off => Vec::new(),
        ChartPlan::Single => single_chart(snap, cfg, theme, width),
        ChartPlan::Facets => facet_charts(snap, cfg, theme, width),
    }
}

/// A money formatter that gives every tick on one axis the same precision.
fn money_axis(currency: String, decimals: usize) -> impl Fn(f64) -> String {
    move |v: f64| fmt::money_with(v, &currency, decimals)
}

fn axis_decimals(values: &[f64]) -> usize {
    let max = values.iter().cloned().fold(0f64, |a, b| a.max(b.abs()));
    fmt::decimals_for(max)
}

fn single_chart(snap: &Snapshot, cfg: &Config, theme: &Theme, width: usize) -> Vec<String> {
    let Some(&idx) = snap.charted.first() else { return Vec::new() };
    let Some(row) = snap.rows.get(idx) else { return Vec::new() };
    let Some(series) = row.series.clone() else { return Vec::new() };
    let values: Vec<f64> = series.iter().map(|(_, v)| *v).collect();
    let y = money_axis(snap.currency.clone(), axis_decimals(&values));
    braille::plot(
        &Plot {
            series: vec![PlotSeries {
                color: row.color,
                points: series,
            }],
            width,
            height: cfg.height,
            range: snap.range,
            y_label: &y,
            gutter_min: 0,
        },
        theme,
    )
}

/// One small chart per coin. Coins of different value cannot share a y-scale
/// usefully, and a shared *percent* scale keeps only three lines apart for a
/// colour-blind reader — so every coin gets its own axis, in money.
fn facet_charts(snap: &Snapshot, cfg: &Config, theme: &Theme, width: usize) -> Vec<String> {
    let charted: Vec<&Row> = snap
        .charted
        .iter()
        .filter_map(|i| snap.rows.get(*i))
        .filter(|r| r.series.as_ref().is_some_and(|s| s.len() > 1))
        .collect();
    if charted.is_empty() {
        return Vec::new();
    }
    let columns = if width >= 120 { 3 } else if width >= 74 { 2 } else { 1 };
    let gap = 3usize;
    let col_w = (width - gap * (columns - 1)) / columns;
    if col_w < 26 {
        return Vec::new();
    }
    // Side by side, facets can afford height; stacked, they must not run off
    // the screen, so each gets less.
    let height = if columns > 1 {
        (cfg.height / 2).clamp(5, 7)
    } else {
        (cfg.height / 3).clamp(4, 5)
    };

    // Rendered once to discover the widest y-label gutter, then again with
    // that gutter shared — otherwise each facet's axis, and the title sitting
    // above it, starts at a different column.
    let gutter = charted
        .iter()
        .map(|row| braille::gutter_of(&facet(row, snap, theme, col_w, height, 0)))
        .max()
        .unwrap_or(0);

    let mut out = Vec::new();
    for chunk in charted.chunks(columns) {
        let blocks: Vec<Vec<String>> = chunk
            .iter()
            .map(|row| facet(row, snap, theme, col_w, height, gutter))
            .collect();
        let widths = vec![col_w; blocks.len()];
        if !out.is_empty() {
            out.push(String::new());
        }
        out.extend(braille::join_columns(&blocks, &widths, gap));
    }
    out
}

fn facet(
    row: &Row,
    snap: &Snapshot,
    theme: &Theme,
    width: usize,
    height: usize,
    gutter: usize,
) -> Vec<String> {
    // Over the plot area rather than over the axis numbers — in a grid this
    // reads better, because the eye compares curves across panels.
    let mut title = SLine::new();
    title.spaces(gutter + 1);
    let chip = format!("● {}", row.market.ticker());
    title.styled(&chip, theme.paint(&chip, theme.series(row.color)));
    title.spaces(2);
    if let Some(p) = row.market.current_price {
        let text = fmt::money(p, &snap.currency);
        title.styled(&text, theme.bold(&text));
    }
    if let Some(v) = row.series.as_deref().and_then(change_over) {
        title.spaces(2);
        let text = fmt::percent(v);
        title.styled(&text, theme.delta(&text, v));
    }
    // A blank line between the title and its plot: flush against the axis the
    // two read as one block, and the title stops looking like a label.
    let mut out = vec![title.finish(), String::new()];
    let Some(series) = row.series.clone() else { return out };
    let values: Vec<f64> = series.iter().map(|(_, v)| *v).collect();
    let y = money_axis(snap.currency.clone(), axis_decimals(&values));
    out.extend(braille::plot(
        &Plot {
            series: vec![PlotSeries {
                color: row.color,
                points: series,
            }],
            width,
            height,
            range: snap.range,
            y_label: &y,
            gutter_min: gutter,
        },
        theme,
    ));
    out
}

// ---------------------------------------------------------------- portfolio ---

/// What is left of the portfolio block once its numbers moved into the table:
/// the allocation bar, which is a shape rather than a row of figures.
fn portfolio_block(
    p: &Portfolio,
    _snap: &Snapshot,
    theme: &Theme,
    width: usize,
) -> Vec<String> {
    let shares = portfolio::shares(p);
    if shares.len() < 2 {
        return Vec::new();
    }
    allocation_bar(&shares, theme, width)
}

/// A one-line part-to-whole bar: at most six segments, a space between each so
/// neighbours never touch, and a labelled chip for every one of them.
fn allocation_bar(
    shares: &[(String, usize, f64)],
    theme: &Theme,
    width: usize,
) -> Vec<String> {
    let bar_w = (width / 2).clamp(20, 44);
    let mut segments: Vec<(String, usize, f64)> = shares.iter().take(6).cloned().collect();
    let tail: f64 = shares.iter().skip(6).map(|(_, _, f)| f).sum();
    if tail > 0.0 {
        segments.push(("other".into(), usize::MAX, tail));
    }

    let mut line = SLine::new();
    for (i, (_, color, frac)) in segments.iter().enumerate() {
        if i > 0 {
            line.text(" ");
        }
        let cells = (frac * bar_w as f64).round().max(1.0) as usize;
        let block = "█".repeat(cells);
        if *color == usize::MAX {
            line.styled(&block, theme.dim(&block));
        } else {
            line.styled(&block, theme.paint(&block, theme.series(*color)));
        }
    }
    let percentages = portfolio::percentages(
        &segments.iter().map(|(_, _, f)| *f).collect::<Vec<_>>(),
    );
    let mut labels = SLine::new();
    for (i, (name, color, _)) in segments.iter().enumerate() {
        labels.text("  ");
        let chip = "●";
        if *color == usize::MAX {
            labels.styled(chip, theme.dim(chip));
        } else {
            labels.styled(chip, theme.paint(chip, theme.series(*color)));
        }
        let pct = percentages.get(i).cloned().unwrap_or_default();
        labels.text(&format!(" {name} {pct}%"));
    }
    // Side by side when they fit, stacked when they don't.
    if line.width() + labels.width() <= width {
        let mut joined = SLine::new();
        joined.text(&line.finish());
        joined.text(&labels.finish());
        vec![joined.finish()]
    } else {
        vec![line.finish(), labels.finish().trim_start().to_string()]
    }
}
