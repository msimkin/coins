//! The price table: one row per tracked coin, columns sized to their content.
//!
//! Identity is carried by a coloured `●` next to the ticker, not by colouring
//! the text — so the numbers stay legible ink, and the same hue as the chart
//! ties each row to its line.

use crate::config::Config;
use crate::data::{Snapshot, View};
use crate::render::fmt;
use crate::render::theme::{SLine, Theme, vis_width};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Align {
    Left,
    Right,
}

/// The gap between one column and the next. Shared, because the rule under a
/// totals row has to cross exactly the gaps the columns leave.
const GAP: usize = 2;

#[derive(Debug, Clone)]
enum Cell {
    Plain(String),
    Dim(String),
    Bold(String),
    /// A percentage, coloured by its sign.
    Delta(String, f64),
    /// Text in a series colour — only the identity chip uses this.
    Chip(String, usize),
    /// A trend sparkline in the coin's colour.
    Trend(String, usize),
}

impl Cell {
    fn text(&self) -> String {
        match self {
            Cell::Plain(s) | Cell::Dim(s) | Cell::Bold(s) => s.clone(),
            Cell::Delta(s, _) | Cell::Chip(s, _) | Cell::Trend(s, _) => s.clone(),
        }
    }

    /// The cell's style, applied to any text.
    fn paint_str(&self, s: &str, theme: &Theme) -> String {
        match self {
            Cell::Plain(_) => s.to_string(),
            Cell::Dim(_) => theme.dim(s),
            Cell::Bold(_) => theme.bold(s),
            Cell::Delta(_, v) => theme.delta(s, *v),
            Cell::Chip(_, c) | Cell::Trend(_, c) => theme.paint(s, theme.series(*c)),
        }
    }

    fn paint(&self, theme: &Theme) -> String {
        self.paint_str(&self.text(), theme)
    }
}

struct Column {
    align: Align,
}

/// A group of rows: the coins, then the things the tracked addresses hold.
///
/// The leading columns are shared and sized across every group — that is what
/// makes the change columns line up. What comes *after* them is the group's
/// own: a plot for the coins, an amount and an address for the holdings. Those
/// are different data, so forcing them into one shared column only produced a
/// hole in whichever group was not using it.
/// A hairline over the columns a totals row adds up, drawn before `before_row`
/// and starting at `from_column` — so it covers the money and the changes, and
/// stops before the columns that are not summed.
#[derive(Clone, Copy)]
struct Rule {
    before_row: usize,
    from_column: usize,
}

struct Section {
    /// Labels for the shared columns.
    header: Vec<String>,
    /// Per-column alignment where it differs from the column's own.
    align: Vec<Option<Align>>,
    rows: Vec<Vec<Cell>>,
    /// This group's own columns, appended after the shared ones.
    tail_header: Vec<String>,
    tail_align: Vec<Align>,
    /// One entry per row, each the same length as `tail_header`.
    tail_rows: Vec<Vec<Cell>>,
    /// Set only where a row sums the rows above it.
    rule: Option<Rule>,
}

/// Narrowest plot worth drawing, and the point past which a wider one stops
/// telling you more.
const MIN_PLOT: usize = 10;
const MAX_PLOT: usize = 32;

/// A rendered table, and whether the holdings groups survived the width. The
/// allocation bar belongs to those groups, so it must not be drawn without them.
pub struct Rendered {
    pub lines: Vec<String>,
    pub with_addresses: bool,
}

/// Renders the table. The per-row plot takes whatever width the other columns
/// leave, so it grows with the terminal instead of sitting at a fixed size.
/// The narrowest a name column is worth having. Below this it says nothing that
/// the ticker beside it has not already said.
const MIN_NAME: usize = 6;

/// How the market list spends whatever width the figures leave on names.
///
/// Three outcomes, in the order they are preferred: every name whole; the column
/// capped, so the long ones end in `…` and the rest are untouched; or no column
/// at all, when so little is left that most names would be stumps.
fn name_style(snap: &Snapshot, cfg: &Config, width: usize) -> Style {
    let bare = Style { with_name: false, name_cap: None, tight: false, with_addresses: false };
    let lengths: Vec<usize> = snap
        .rows
        .iter()
        .map(|r| vis_width(&fmt::clean_text(&r.market.name)))
        .collect();
    let room = width.saturating_sub(measure(snap, cfg, bare).widest + GAP);
    match name_plan(&lengths, room) {
        NamePlan::None => bare,
        NamePlan::Whole => Style { with_name: true, ..bare },
        NamePlan::Cut(cap) => Style { with_name: true, name_cap: Some(cap), ..bare },
    }
}

#[derive(Debug, PartialEq, Eq)]
enum NamePlan {
    Whole,
    Cut(usize),
    None,
}

/// What to do with the names, given the room left for them: all of them whole,
/// the column cut to what there is, or no column at all. Most of them readable
/// is worth a few ellipses; most of them cut to stumps is not worth the column.
fn name_plan(lengths: &[usize], room: usize) -> NamePlan {
    let Some(&longest) = lengths.iter().max() else { return NamePlan::None };
    if room >= longest {
        return NamePlan::Whole;
    }
    let fit = lengths.iter().filter(|l| **l <= room).count();
    if room >= MIN_NAME && fit * 2 >= lengths.len() {
        return NamePlan::Cut(room);
    }
    NamePlan::None
}

pub fn table(snap: &Snapshot, cfg: &Config, theme: &Theme, width: usize) -> Rendered {
    if snap.view == View::Top {
        return emitted(snap, cfg, theme, name_style(snap, cfg, width), 0);
    }
    // Degrade in order of what the screen is for. The address rows outrank the
    // coin's full name and the plot, but they add two columns, so on a narrow
    // terminal they go compact and then go entirely rather than overflow.
    for with_addresses in [true, false] {
        let tights: &[bool] = if with_addresses { &[false, true] } else { &[false] };
        for &tight in tights {
            for with_name in [true, false] {
                let style = Style { with_name, name_cap: None, tight, with_addresses };
                let m = measure(snap, cfg, style);
                if m.widest > width {
                    continue;
                }
                // Only the coin rows carry a plot, so its budget is what is
                // left on *their* line — the address columns no longer compete
                // for the same space.
                let plot = if cfg.inline_plot && snap.show_coins && snap.view != View::Top {
                    width.saturating_sub(m.coins + GAP).min(MAX_PLOT)
                } else {
                    0
                };
                let plot = if plot >= MIN_PLOT { plot } else { 0 };
                return emitted(snap, cfg, theme, style, plot);
            }
        }
    }
    // Nothing fits: the prices matter more than the terminal's feelings.
    emitted(snap, cfg, theme, Style::bare(), 0)
}

fn emitted(
    snap: &Snapshot,
    cfg: &Config,
    theme: &Theme,
    style: Style,
    plot: usize,
) -> Rendered {
    let (columns, sections) = build(snap, cfg, style, plot);
    let shared = widths(&columns, &sections);
    Rendered {
        lines: emit(&columns, &sections, &shared, theme),
        // More than the coins group means the holdings groups are present.
        with_addresses: sections.len() > 1,
    }
}

/// Line widths with no plot drawn: the coin rows, and the widest row of any
/// group.
struct Measured {
    coins: usize,
    widest: usize,
}

fn measure(snap: &Snapshot, cfg: &Config, style: Style) -> Measured {
    let (columns, sections) = build(snap, cfg, style, 0);
    let shared = widths(&columns, &sections);
    let prefix: usize = shared.iter().sum::<usize>() + GAP * shared.len().saturating_sub(1);
    let mut widest = 0;
    let mut coins = 0;
    for (i, section) in sections.iter().enumerate() {
        let tail = tail_widths(section);
        let mut total = prefix;
        for w in &tail {
            total += GAP + w;
        }
        if i == 0 {
            coins = total;
        }
        widest = widest.max(total);
    }
    Measured { coins, widest }
}

fn tail_widths(section: &Section) -> Vec<usize> {
    (0..section.tail_header.len())
        .map(|i| {
            let body = section
                .tail_rows
                .iter()
                .filter_map(|r| r.get(i))
                .map(|c| vis_width(&c.text()))
                .max()
                .unwrap_or(0);
            body.max(vis_width(&section.tail_header[i]))
        })
        .collect()
}

/// Which optional pieces a layout attempt includes.
#[derive(Debug, Clone, Copy)]
struct Style {
    with_name: bool,
    /// The market list's name column, capped to this many characters. `None`
    /// leaves the names whole; the other views do not use it.
    name_cap: Option<usize>,
    /// Shorter wallet labels and coarser amounts, to keep the address rows on
    /// a narrow terminal instead of dropping them.
    tight: bool,
    with_addresses: bool,
}

impl Style {
    fn bare() -> Style {
        Style { with_name: false, name_cap: None, tight: true, with_addresses: false }
    }
}

/// The holdings to show, flattened to one entry per (source, coin).
type Holding<'a> = (&'a crate::portfolio::HoldingSource, &'a String, f64);

fn holdings_of(snap: &Snapshot, with_addresses: bool) -> Vec<Holding<'_>> {
    let sources: &[crate::portfolio::HoldingSource] = match &snap.portfolio {
        Some(p) if snap.show_addresses && with_addresses => &p.sources,
        _ => &[],
    };
    sources
        .iter()
        .flat_map(|s| s.coins.iter().map(move |(id, a)| (s, id, *a)))
        .collect()
}

/// One row per tracked coin: identity, name, price, changes — and, in its own
/// trailing columns, the figures that have no address analogue.
fn coins_section(
    snap: &Snapshot,
    cfg: &Config,
    style: Style,
    trend_cells: usize,
    col_two: bool,
    decimals: usize,
) -> Section {
    let changes = cfg.change_columns();
    let trend_is_range = snap.trend_is_range();

    let mut header = vec!["COINS".to_string()];
    if col_two {
        header.push(String::new());
    }
    header.push("PRICE".to_string());
    for c in &changes {
        header.push(c.to_ascii_uppercase());
    }

    // The plot belongs to this group rather than to the shared grid: an
    // address row has nothing to put there, and a shared blank column would
    // leave a hole between the changes and the amount.
    let mut tail_header = Vec::new();
    let mut tail_align = Vec::new();
    if trend_cells > 0 {
        tail_header.push(if trend_is_range {
            snap.range.label().to_ascii_uppercase()
        } else {
            "7 DAYS".to_string()
        });
        tail_align.push(Align::Left);
    }

    let mut rows = Vec::new();
    let mut tail_rows = Vec::new();
    for row in &snap.rows {
        let m = &row.market;
        let mut cells = vec![Cell::Chip(format!("● {}", m.ticker()), row.color)];
        if col_two {
            cells.push(Cell::Dim(if style.with_name {
                fmt::clean_text(&m.name)
            } else {
                String::new()
            }));
        }
        cells.push(match m.current_price {
            Some(p) => Cell::Bold(fmt::money_with(p, &snap.currency, decimals)),
            None => Cell::Dim("·".into()),
        });
        cells.extend(changes.iter().map(|c| delta_cell(row.change(c))));

        let mut tail = Vec::new();
        if trend_cells > 0 {
            let prices = trend_prices(row, trend_is_range);
            tail.push(Cell::Trend(fmt::sparkline(&prices, trend_cells), row.color));
        }
        rows.push(cells);
        tail_rows.push(tail);
    }
    Section {
        header,
        align: vec![None; 3 + changes.len()],
        rows,
        tail_header,
        tail_align,
        tail_rows,
        // Nothing in the coins group sums the rows above it.
        rule: None,
    }
}

/// The market, largest first: rank, coin, name, price, what it is all worth, and
/// the same change columns as everywhere else.
///
/// The chips are ink rather than colour, except for the coins in `coins` — the
/// palette holds six hues and clamps past them, so fifty coloured rows would be
/// forty-five identical dots, and "where do mine sit" is the only question this
/// screen answers that a website does not.
fn top_section(snap: &Snapshot, cfg: &Config, style: Style, decimals: usize) -> Section {
    // Only the periods the prices request answers: a `3m` or `6m` column is a
    // chart per coin, and fifty of those is not a screen anyone should pay for.
    let changes = cfg.market_columns();
    let mut header = vec!["#".to_string(), "COIN".to_string()];
    if style.with_name {
        header.push("NAME".to_string());
    }
    header.push("PRICE".to_string());
    // `VALUE` rather than `MARKET CAP`: the figures under it are six characters
    // wide and the longer heading spent five more on every row, which is what
    // pushed the names off a narrow screen.
    header.push("VALUE".to_string());
    for c in &changes {
        header.push(c.to_ascii_uppercase());
    }
    let mut rows = Vec::new();
    let blank = |n: usize| vec![Cell::Plain(String::new()); n];
    for (i, row) in snap.rows.iter().enumerate() {
        // Where the ranked coins end and your own, from further down, begin.
        if Some(i) == snap.top_break {
            let width = rows.last().map_or(0, |r: &Vec<Cell>| r.len());
            rows.push(blank(width));
        }
        let m = &row.market;
        // One `FIGR_HELOC` among fifty coins would widen this column for all of
        // them, and it is the names that pay for it. Tickers are five characters
        // or fewer with very few exceptions, so the exceptions are cut instead.
        let chip = format!("● {}", shorten(&m.ticker(), 6));
        let mut cells = vec![
            match m.market_cap_rank {
                Some(r) => Cell::Dim(r.to_string()),
                None => Cell::Dim("·".into()),
            },
            // Three levels and no new colour: yours, ordinary, and pegged.
            if cfg.coins.iter().any(|c| c == &m.id) {
                Cell::Chip(chip, row.color)
            } else if crate::coingecko::is_pegged(m) {
                Cell::Dim(chip)
            } else {
                Cell::Plain(chip)
            },
        ];
        if style.with_name {
            let name = fmt::clean_text(&m.name);
            cells.push(Cell::Dim(match style.name_cap {
                Some(cap) => shorten(&name, cap),
                None => name,
            }));
        }
        cells.push(match m.current_price {
            Some(p) => Cell::Bold(fmt::money_with(p, &snap.currency, decimals)),
            None => Cell::Dim("·".into()),
        });
        cells.push(match m.market_cap {
            Some(c) => Cell::Plain(fmt::compact(c, &snap.currency)),
            None => Cell::Dim("·".into()),
        });
        cells.extend(changes.iter().map(|c| delta_cell(row.change(c))));
        rows.push(cells);
    }
    let width = rows.first().map_or(0, |r| r.len());
    Section {
        header,
        align: vec![None; width],
        rows,
        // No sparklines here whatever `inline_plot` says: fifty of them beside
        // fifty ranks is a wall of ink, and this screen is for reading places
        // in a market, not shapes.
        tail_header: Vec::new(),
        tail_align: Vec::new(),
        tail_rows: Vec::new(),
        rule: None,
    }
}

/// One row per holding, then the group's totals row. `None` when nothing is
/// held, or when no held coin has a row to take its price from.
fn holdings_section(
    snap: &Snapshot,
    cfg: &Config,
    style: Style,
    holdings: &[Holding<'_>],
    col_two: bool,
    values: usize,
) -> Option<Section> {
    if holdings.is_empty() {
        return None;
    }
    let changes = cfg.change_columns();
    let price_of = |id: &str| price_of(snap, id);

    let mut header = vec!["ADDRESSES".to_string()];
    if col_two {
        header.push(String::new());
    }
    header.push("VALUE".to_string());
    for c in &changes {
        header.push(c.to_ascii_uppercase());
    }

    let mut rows = Vec::new();
    let mut tail_rows = Vec::new();
    for (source, id, amount) in holdings {
        let Some(row) = snap.rows.iter().find(|r| &r.market.id == *id) else { continue };
        let m = &row.market;
        let mut cells = vec![Cell::Chip(format!("● {}", m.ticker()), row.color)];
        if col_two {
            cells.push(Cell::Dim(shorten(
                &source.label,
                if style.tight { 8 } else { 20 },
            )));
        }
        cells.push(Cell::Bold(fmt::money_with(
            amount * price_of(id),
            &snap.currency,
            values,
        )));
        // A holding's amount is fixed over the period, so its change is its
        // coin's change — the same figure, in the same column.
        cells.extend(changes.iter().map(|c| delta_cell(row.change(c))));

        rows.push(cells);
        tail_rows.push(vec![
            Cell::Plain(fmt::amount(*amount)),
            Cell::Dim(match &source.address {
                Some(a) => crate::wallet::short_address(a),
                None => String::new(),
            }),
        ]);
    }
    if rows.is_empty() {
        return None;
    }

    // The totals row closes the group. Not a group of its own: a separate block
    // would need its own header to label the change columns, and a row sitting
    // directly under the ones it adds up is already labelled by them. A single
    // holding is its own total, so it gets no summary.
    let mut rule = None;
    if rows.len() > 1 {
        let total: f64 = holdings
            .iter()
            .map(|(_, id, amount)| amount * price_of(id))
            .sum();
        // Set here rather than after the fact, so the rule cannot outlive the
        // row it belongs to: the money column is where the total's own figure
        // goes, two columns in when the labels have a column of their own.
        rule = Some(Rule {
            before_row: rows.len(),
            from_column: if col_two { 2 } else { 1 },
        });
        let mut cells = vec![Cell::Plain(String::new())];
        if col_two {
            // The coin column stays empty — a total belongs to no single coin —
            // and the word goes where the wallet labels are, which are
            // lower-case words of exactly this kind.
            cells.push(Cell::Dim("total".into()));
        }
        cells.push(Cell::Bold(fmt::money_with(total, &snap.currency, values)));
        cells.extend(
            changes
                .iter()
                .map(|c| delta_cell(weighted_change(snap, holdings, c))),
        );
        rows.push(cells);
        tail_rows.push(vec![Cell::Plain(String::new()), Cell::Plain(String::new())]);
    }

    Some(Section {
        header,
        align: vec![None; 3 + changes.len()],
        rows,
        tail_header: vec!["AMOUNT".into(), "ADDRESS".into()],
        tail_align: vec![Align::Right, Align::Left],
        tail_rows,
        rule,
    })
}

/// The portfolio's change for one period, weighted by value over the holdings
/// that have a figure — so a coin missing one shrinks the basket rather than
/// skewing it.
fn weighted_change(snap: &Snapshot, holdings: &[Holding<'_>], column: &str) -> Option<f64> {
    let mut sum = 0.0;
    let mut covered = 0.0;
    for (_, id, amount) in holdings {
        let value = amount * price_of(snap, id);
        let change = snap
            .rows
            .iter()
            .find(|r| &r.market.id == *id)
            .and_then(|r| r.change(column))
            .filter(|c| c.is_finite());
        if let Some(c) = change {
            sum += value * c;
            covered += value;
        }
    }
    (covered > 0.0).then(|| sum / covered)
}

fn price_of(snap: &Snapshot, id: &str) -> f64 {
    snap.rows
        .iter()
        .find(|r| r.market.id == id)
        .and_then(|r| r.market.current_price)
        .unwrap_or(0.0)
}

fn delta_cell(change: Option<f64>) -> Cell {
    match change {
        Some(v) if v.is_finite() => Cell::Delta(fmt::percent(v), v),
        _ => Cell::Dim("·".into()),
    }
}

fn build(
    snap: &Snapshot,
    cfg: &Config,
    style: Style,
    trend_cells: usize,
) -> (Vec<Column>, Vec<Section>) {
    let changes = cfg.change_columns();
    // The market list is a different shape: a rank in front, a capitalisation
    // after the price, and no group that could hold an address.
    if snap.view == View::Top {
        let changes = cfg.market_columns();
        let decimals =
            fmt::column_decimals(snap.rows.iter().filter_map(|r| r.market.current_price));
        let mut columns = vec![Column { align: Align::Right }, Column { align: Align::Left }];
        if style.with_name {
            columns.push(Column { align: Align::Left });
        }
        columns.push(Column { align: Align::Right });
        columns.push(Column { align: Align::Right });
        columns.extend(changes.iter().map(|_| Column { align: Align::Right }));
        return (columns, vec![top_section(snap, cfg, style, decimals)]);
    }
    let holdings = holdings_of(snap, style.with_addresses);
    // The second column holds a coin's name among the coins and the wallet's
    // label among the addresses, so it exists if either wants it.
    let col_two = style.with_name || !holdings.is_empty();

    // The shared columns: identity, name or label, money, then the changes.
    // These are the ones that must line up between the groups.
    let mut columns = vec![Column { align: Align::Left }];
    if col_two {
        columns.push(Column { align: Align::Left });
    }
    columns.push(Column { align: Align::Right });
    columns.extend(changes.iter().map(|_| Column { align: Align::Right }));

    // One decimal count per column, the largest any of its rows needs, so the
    // decimal marks line up without a special alignment mode.
    let decimals = fmt::column_decimals(snap.rows.iter().filter_map(|r| r.market.current_price));
    let values = fmt::column_decimals(
        holdings
            .iter()
            .map(|(_, id, amount)| amount * price_of(snap, id)),
    );

    let mut sections = Vec::new();
    if snap.show_coins {
        sections.push(coins_section(
            snap, cfg, style, trend_cells, col_two, decimals,
        ));
    }
    if let Some(section) = holdings_section(snap, cfg, style, &holdings, col_two, values) {
        sections.push(section);
    }
    (columns, sections)
}

/// Cuts a label to `max` characters, marking where it was cut.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// The values behind a row's sparkline. When any row lacks history for the
/// selected range, every row falls back to the 7-day sparkline so one header
/// can't describe two different periods.
fn trend_prices(row: &crate::data::Row, trend_is_range: bool) -> Vec<f64> {
    if trend_is_range {
        if let Some(s) = &row.series {
            return s.iter().map(|(_, v)| *v).collect();
        }
    }
    row.market
        .sparkline_in_7d
        .as_ref()
        .map(|s| s.price.clone())
        .unwrap_or_default()
}

fn widths(columns: &[Column], sections: &[Section]) -> Vec<usize> {
    (0..columns.len())
        .map(|i| {
            let cells: Vec<&Cell> = sections
                .iter()
                .flat_map(|s| s.rows.iter())
                .filter_map(|r| r.get(i))
                .collect();
            let header = sections
                .iter()
                .filter_map(|s| s.header.get(i))
                .map(|h| vis_width(h))
                .max()
                .unwrap_or(0);
            let body = cells.iter().map(|c| vis_width(&c.text())).max().unwrap_or(0);
            body.max(header)
        })
        .collect()
}

/// Where a rule under the summed columns starts, and how far it runs: from the
/// left edge of `from` to the right edge of the last shared column, crossing the
/// two-space gaps between them on the way.
fn rule_span(shared: &[usize], from: usize) -> (usize, usize) {
    let start = shared[..from].iter().sum::<usize>() + GAP * from;
    let len = shared[from..].iter().sum::<usize>() + GAP * (shared.len() - from - 1);
    (start, len)
}

fn emit(
    columns: &[Column],
    sections: &[Section],
    shared: &[usize],
    theme: &Theme,
) -> Vec<String> {
    let mut out = Vec::new();
    for (n, section) in sections.iter().enumerate() {
        if n > 0 {
            out.push(String::new());
        }
        let tail = tail_widths(section);

        let mut header = SLine::new();
        for (i, col) in columns.iter().enumerate() {
            if i > 0 {
                header.spaces(GAP);
            }
            let text = section.header.get(i).cloned().unwrap_or_default();
            let align = section.align.get(i).copied().flatten().unwrap_or(col.align);
            pad_cell(&mut header, &text, theme.dim(&text), shared[i], align);
        }
        for (i, text) in section.tail_header.iter().enumerate() {
            header.spaces(GAP);
            pad_cell(&mut header, text, theme.dim(text), tail[i], section.tail_align[i]);
        }
        out.push(header.finish().trim_end().to_string());

        for (r, row) in section.rows.iter().enumerate() {
            // A hairline over the figures this row adds up, in the grey the
            // header rule and the plot rails use: structure, not a divider.
            if let Some(rule) = section.rule.filter(|x| x.before_row == r) {
                let (start, len) = rule_span(shared, rule.from_column);
                let bar = "─".repeat(len);
                let mut line = SLine::new();
                line.spaces(start);
                line.styled(&bar, theme.paint(&bar, theme.axis()));
                out.push(line.finish());
            }
            let mut line = SLine::new();
            for (i, cell) in row.iter().enumerate() {
                if i > 0 {
                    line.spaces(GAP);
                }
                let align = section.align.get(i).copied().flatten().unwrap_or(columns[i].align);
                let text = cell.text();
                pad_cell(&mut line, &text, cell.paint(theme), shared[i], align);
            }
            if let Some(cells) = section.tail_rows.get(r) {
                for (i, cell) in cells.iter().enumerate() {
                    line.spaces(GAP);
                    let text = cell.text();
                    pad_cell(&mut line, &text, cell.paint(theme), tail[i], section.tail_align[i]);
                }
            }
            out.push(line.finish().trim_end().to_string());
        }
    }
    out
}

/// Writes one cell, padded to `width` on the side its alignment asks for.
fn pad_cell(line: &mut SLine, text: &str, painted: String, width: usize, align: Align) {
    let pad = width.saturating_sub(vis_width(text));
    if align == Align::Right {
        line.spaces(pad);
    }
    line.styled(text, painted);
    if align == Align::Left {
        line.spaces(pad);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_kept_whole_cut_or_dropped_by_the_room_there_is() {
        let names = [7, 8, 8, 11, 12, 20];
        // Room for the longest: nothing is cut.
        assert_eq!(name_plan(&names, 20), NamePlan::Whole);
        assert_eq!(name_plan(&names, 25), NamePlan::Whole);
        // Room for most of them: the long ones lose their tails.
        assert_eq!(name_plan(&names, 11), NamePlan::Cut(11));
        assert_eq!(name_plan(&names, 8), NamePlan::Cut(8));
        // Room for hardly any: the column is worth less than the width it takes.
        assert_eq!(name_plan(&names, 7), NamePlan::None);
        assert_eq!(name_plan(&names, 5), NamePlan::None);
        assert_eq!(name_plan(&names, 0), NamePlan::None);
        assert_eq!(name_plan(&[], 40), NamePlan::None);
    }

    #[test]
    fn a_rule_covers_the_summed_columns_and_no_others() {
        // chip, label, money, and three change columns.
        let shared = [6, 11, 11, 6, 6, 7];
        let (start, len) = rule_span(&shared, 2);
        // It begins where the money column begins: two columns and their gaps in.
        assert_eq!(start, 6 + 11 + GAP * 2);
        // And ends with the last change column, gaps between them included.
        assert_eq!(start + len, shared.iter().sum::<usize>() + GAP * (shared.len() - 1));
    }

    #[test]
    fn a_table_without_a_name_column_starts_the_rule_one_column_earlier() {
        let shared = [6, 11, 6, 6];
        let (start, len) = rule_span(&shared, 1);
        assert_eq!(start, 6 + GAP);
        assert_eq!(start + len, shared.iter().sum::<usize>() + GAP * (shared.len() - 1));
    }
}
