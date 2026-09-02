//! A braille canvas and the chart drawn on it.
//!
//! Each terminal cell carries a 2×4 grid of dots, so a chart gets eight times
//! the vertical resolution and twice the horizontal resolution of a character
//! plot — which is the whole reason the curves come out smooth. Every cell is
//! owned by whichever series put the most dots in it, and that decides its
//! colour.

use crate::config::Range;
use crate::render::fmt;
use crate::render::theme::{SLine, Theme, vis_width};

const BRAILLE_BASE: u32 = 0x2800;
/// Bit *index* for (column 0-1, row 0-3) in the Unicode braille encoding.
/// The low six bits are the 3×2 dots; bits 6 and 7 are the extra bottom row.
const DOT_BITS: [[u8; 4]; 2] = [[0, 1, 2, 6], [3, 4, 5, 7]];
const MAX_SERIES: usize = 8;

#[derive(Clone, Copy, Default)]
struct Cell {
    bits: u8,
    counts: [u8; MAX_SERIES],
}

pub struct Canvas {
    w: usize,
    h: usize,
    cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas { w, h, cells: vec![Cell::default(); w * h] }
    }

    /// Dot-space dimensions.
    pub fn dot_w(&self) -> i32 {
        (self.w * 2) as i32
    }
    pub fn dot_h(&self) -> i32 {
        (self.h * 4) as i32
    }

    fn set(&mut self, x: i32, y: i32, series: usize) {
        if x < 0 || y < 0 || x >= self.dot_w() || y >= self.dot_h() {
            return;
        }
        let (cx, cy) = (x as usize / 2, y as usize / 4);
        let cell = &mut self.cells[cy * self.w + cx];
        cell.bits |= 1 << DOT_BITS[x as usize % 2][y as usize % 4];
        let s = series.min(MAX_SERIES - 1);
        cell.counts[s] = cell.counts[s].saturating_add(1);
    }

    #[cfg(test)]
    fn dot(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x >= self.dot_w() || y >= self.dot_h() {
            return false;
        }
        let (cx, cy) = (x as usize / 2, y as usize / 4);
        self.cells[cy * self.w + cx].bits & (1 << DOT_BITS[x as usize % 2][y as usize % 4]) != 0
    }

    /// Bresenham, so a series with fewer points than columns still reads as a
    /// continuous line rather than a dotted one.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, series: usize) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y) = (x0, y0);
        let mut err = dx + dy;
        loop {
            self.set(x, y, series);
            // A single dot is a hairline at this scale; a second one below it
            // doubles the line's visual weight without blurring its shape.
            self.set(x, y + 1, series);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// One row of cells as (glyph, owning series).
    fn row(&self, cy: usize) -> Vec<(char, Option<usize>)> {
        (0..self.w)
            .map(|cx| {
                let cell = self.cells[cy * self.w + cx];
                if cell.bits == 0 {
                    return (' ', None);
                }
                let glyph = char::from_u32(BRAILLE_BASE + cell.bits as u32).unwrap_or(' ');
                let owner = cell
                    .counts
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| **n > 0)
                    .max_by_key(|(i, n)| (**n, usize::MAX - i))
                    .map(|(i, _)| i);
                (glyph, owner)
            })
            .collect()
    }
}

pub struct PlotSeries {
    /// Index into the theme's series palette.
    pub color: usize,
    pub points: Vec<(i64, f64)>,
}

pub struct Plot<'a> {
    pub series: Vec<PlotSeries>,
    /// Total columns available, including the y-label gutter.
    pub width: usize,
    /// Plot rows, excluding the axis and label lines.
    pub height: usize,
    pub range: Range,
    /// How a y value is written in the gutter.
    pub y_label: &'a dyn Fn(f64) -> String,
    /// Least width for the y-label gutter. Facets pass a shared value so their
    /// axes — and the titles above them — line up across the grid.
    pub gutter_min: usize,
}

/// Renders the chart into terminal lines, ANSI included.
/// What a plot covers, after padding and any baseline that must be on screen.
struct Bounds {
    lo: f64,
    hi: f64,
    t0: i64,
    t1: i64,
}

/// The extent of the data, padded so curves do not graze the frame. `None`
/// when there is nothing finite to draw, or no time span to draw it over.
fn bounds(points: &[&PlotSeries]) -> Option<Bounds> {
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut t0, mut t1) = (i64::MAX, i64::MIN);
    for s in points {
        for (t, v) in &s.points {
            if !v.is_finite() {
                continue;
            }
            lo = lo.min(*v);
            hi = hi.max(*v);
            t0 = t0.min(*t);
            t1 = t1.max(*t);
        }
    }
    if !lo.is_finite() || !hi.is_finite() || t1 <= t0 {
        return None;
    }
    if (hi - lo).abs() < f64::EPSILON {
        let pad = if hi.abs() > 0.0 { hi.abs() * 0.01 } else { 1.0 };
        lo -= pad;
        hi += pad;
    } else {
        let pad = (hi - lo) * 0.04;
        lo -= pad;
        hi += pad;
    }
    Some(Bounds { lo, hi, t0, t1 })
}

/// How the row is divided: the label gutter, and the plot area after it.
struct Layout {
    gutter: usize,
    plot_w: usize,
}

fn layout(spec: &Plot, label_w: usize) -> Option<Layout> {
    // One space between the widest label and the axis, so numbers don't touch it.
    let gutter = (label_w + 1).max(spec.gutter_min);
    let plot_w = spec.width.saturating_sub(gutter + 1);
    (plot_w >= 12).then_some(Layout { gutter, plot_w })
}

/// Inks every series, and reports where each one ended so its name can go there.
fn draw(
    spec: &Plot,
    points: &[&PlotSeries],
    b: &Bounds,
    at: &Layout,
) -> Canvas {
    // Two dots thick: a single dot is a hairline at this scale.
    let mut canvas = Canvas::new(at.plot_w, spec.height);
    let (dw, dh) = (canvas.dot_w(), canvas.dot_h());
    for s in points {
        let mut prev: Option<(i32, i32)> = None;
        // One value per dot column: without this, several points share a column
        // and the line becomes a row of vertical strokes rather than a curve.
        for (x, v) in columnise(&s.points, b.t0, b.t1, dw) {
            let y = ((b.hi - v) / (b.hi - b.lo) * f64::from(dh - 1)).round() as i32;
            let y = y.clamp(0, dh - 1);
            match prev {
                Some((px, py)) => canvas.line(px, py, x, y, s.color),
                None => canvas.line(x, y, x, y, s.color),
            }
            prev = Some((x, y));
        }
    }

    // The wash lives on its own canvas so the line keeps its full hue on top.
    // Single series only: under two overlaid lines a fill hides one of them.
    canvas
}

/// Renders the chart into terminal lines, ANSI included.
pub fn plot(spec: &Plot, theme: &Theme) -> Vec<String> {
    let points: Vec<&PlotSeries> = spec.series.iter().filter(|s| s.points.len() > 1).collect();
    if points.is_empty() || spec.width < 24 {
        return Vec::new();
    }
    let Some(b) = bounds(&points) else {
        return Vec::new();
    };

    // More labelled rows: a chart whose axis is mostly bare `┤` makes the
    // reader interpolate, which is how "what am I even looking at" starts.
    let ticks = nice_ticks(b.lo, b.hi, (spec.height / 2).clamp(3, 6));
    let labels: Vec<(usize, String)> = ticks
        .iter()
        .map(|t| (row_of(*t, b.lo, b.hi, spec.height), (spec.y_label)(*t)))
        .collect();
    let label_w = labels.iter().map(|(_, l)| vis_width(l)).max().unwrap_or(0);
    let Some(at) = layout(spec, label_w) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let canvas = draw(spec, &points, &b, &at);
    for r in 0..spec.height {
        out.push(plot_row(r, &canvas, &labels, at.gutter, theme));
    }

    let mut axis = SLine::new();
    axis.spaces(at.gutter);
    let rule = format!("└{}", "─".repeat(at.plot_w));
    axis.styled(&rule, theme.paint(&rule, theme.axis()));
    out.push(axis.finish());
    out.push(x_axis_labels(
        b.t0,
        b.t1,
        spec.range,
        at.gutter + 1,
        at.plot_w,
        theme,
    ));
    out
}

/// One row: its y-label if it carries a tick, the axis, then the inked cells.
fn plot_row(
    r: usize,
    canvas: &Canvas,
    labels: &[(usize, String)],
    gutter: usize,
    theme: &Theme,
) -> String {
    let mut line = SLine::new();
    match labels.iter().find(|(row, _)| *row == r).map(|(_, l)| l) {
        Some(l) => {
            line.spaces(gutter - vis_width(l) - 1);
            line.styled(l, theme.dim(l));
            line.text(" ");
        }
        None => {
            line.spaces(gutter);
        }
    }
    line.styled("┤", theme.paint("┤", theme.axis()));

    // Consecutive cells owned by the same series share one escape sequence:
    // per-cell colouring would emit thousands of them per chart.
    let cells = canvas.row(r);
    let mut i = 0;
    while i < cells.len() {
        let owner = cells[i].1;
        let mut run = String::new();
        while i < cells.len() && cells[i].1 == owner {
            run.push(cells[i].0);
            i += 1;
        }
        match owner {
            Some(c) => line.styled(&run, theme.paint(&run, theme.series(c))),
            None => line.text(&run),
        };
    }
    line.finish()
}

/// Up to five time labels across the axis, dropped rather than allowed to collide.
fn x_axis_labels(
    t0: i64,
    t1: i64,
    range: Range,
    offset: usize,
    plot_w: usize,
    theme: &Theme,
) -> String {
    let count = (plot_w / 16).clamp(2, 5);
    let mut row = vec![' '; offset + plot_w];
    let mut last_end = 0usize;
    for i in 0..count {
        let frac = i as f64 / (count - 1) as f64;
        let t = t0 + ((t1 - t0) as f64 * frac) as i64;
        let text = fmt::time_label(t, range);
        if text.is_empty() {
            continue;
        }
        let w = vis_width(&text);
        // First label starts at the axis, last one ends at it, rest are centred.
        let ideal = offset as f64 + frac * (plot_w - 1) as f64 - frac * w as f64;
        let start = (ideal.round().max(0.0) as usize).min(row.len().saturating_sub(w));
        if start < last_end {
            continue;
        }
        for (j, c) in text.chars().enumerate() {
            row[start + j] = c;
        }
        last_end = start + w + 2;
    }
    let text: String = row.into_iter().collect();
    let text = text.trim_end().to_string();
    if text.is_empty() {
        return String::new();
    }
    theme.dim(&text)
}

/// Averages the series into one value per dot column, keeping only the columns
/// that actually hold data — the gaps between them are bridged by the line.
fn columnise(points: &[(i64, f64)], t0: i64, t1: i64, dots: i32) -> Vec<(i32, f64)> {
    let span = (t1 - t0) as f64;
    let mut acc = vec![(0f64, 0usize); dots as usize];
    for (t, v) in points {
        if !v.is_finite() {
            continue;
        }
        let x = ((*t - t0) as f64 / span * (dots - 1) as f64).round();
        let x = (x.max(0.0) as usize).min(dots as usize - 1);
        acc[x].0 += *v;
        acc[x].1 += 1;
    }
    acc.into_iter()
        .enumerate()
        .filter(|(_, (_, n))| *n > 0)
        .map(|(i, (sum, n))| (i as i32, sum / n as f64))
        .collect()
}

/// Which plot row a value lands on.
fn row_of(v: f64, lo: f64, hi: f64, height: usize) -> usize {
    if hi <= lo || height == 0 {
        return 0;
    }
    (((hi - v) / (hi - lo)) * (height - 1) as f64)
        .round()
        .clamp(0.0, (height - 1) as f64) as usize
}

/// Tick values on 1/2/2.5/5×10^k steps, so gutter labels are round numbers.
pub fn nice_ticks(lo: f64, hi: f64, target: usize) -> Vec<f64> {
    // partial_cmp keeps a NaN bound from producing an infinite tick loop.
    if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) || target == 0 {
        return vec![lo];
    }
    let raw = (hi - lo) / target as f64;
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    // Thresholds sit above each step so a span only slightly over a power of
    // ten keeps the finer ticks instead of halving their count.
    let step = mag
        * if norm <= 1.5 {
            1.0
        } else if norm <= 3.0 {
            2.0
        } else if norm <= 7.0 {
            5.0
        } else {
            10.0
        };
    let mut ticks = Vec::new();
    let mut v = (lo / step).ceil() * step;
    while v <= hi + step * 1e-9 && ticks.len() < 32 {
        // -0.0 and floating dust both print badly.
        ticks.push(if v.abs() < step * 1e-9 { 0.0 } else { v });
        v += step;
    }
    if ticks.is_empty() {
        ticks.push((lo + hi) / 2.0);
    }
    ticks
}

/// The column the axis sits in, for a plot already rendered.
pub fn gutter_of(lines: &[String]) -> usize {
    for line in lines {
        let plain = strip_ansi(line);
        if let Some(i) = plain.char_indices().position(|(_, c)| matches!(c, '┤' | '┼' | '└')) {
            return i;
        }
    }
    0
}

/// Stacks facet blocks into columns, padding to the visible width so that ANSI
/// escapes can't shift the grid.
pub fn join_columns(blocks: &[Vec<String>], widths: &[usize], gap: usize) -> Vec<String> {
    let rows = blocks.iter().map(|b| b.len()).max().unwrap_or(0);
    (0..rows)
        .map(|r| {
            let mut line = SLine::new();
            for (i, block) in blocks.iter().enumerate() {
                if i > 0 {
                    line.spaces(gap);
                }
                let target = line.width() + widths[i];
                if let Some(text) = block.get(r) {
                    // A rendered line already carries escapes; measure it plainly.
                    let plain = strip_ansi(text);
                    line.styled(&plain, text.clone());
                }
                line.pad_to(target);
            }
            line.finish().trim_end().to_string()
        })
        .collect()
}

/// Visible text of an already-rendered line, for width accounting.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_become_braille() {
        let mut c = Canvas::new(2, 1);
        c.set(0, 0, 0);
        assert_eq!(c.row(0)[0].0, '⠁');
        // x=1 is the right-hand column, so its bottom row is dot 8, not dot 7.
        c.set(1, 3, 0);
        assert_eq!(c.row(0)[0].0, '⢁');
        c.set(0, 3, 0);
        assert_eq!(c.row(0)[0].0, '⣁');
    }

    #[test]
    fn lines_are_continuous() {
        let mut c = Canvas::new(4, 1);
        c.line(0, 0, 7, 3, 0);
        // Every cell along the diagonal is inked.
        assert!(c.row(0).iter().all(|(g, _)| *g != ' '));
    }

    #[test]
    fn a_line_is_two_dots_thick() {
        // One dot is a hairline at this scale, so every point inks the dot below.
        let mut c = Canvas::new(1, 1);
        c.line(0, 0, 0, 0, 0);
        assert!(c.dot(0, 0) && c.dot(0, 1));
    }

    #[test]
    fn cells_belong_to_their_densest_series() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, 1);
        c.set(0, 1, 1);
        c.set(1, 0, 2);
        assert_eq!(c.row(0)[0].1, Some(1));
    }

    #[test]
    fn ticks_are_round() {
        assert_eq!(nice_ticks(0.0, 10.0, 5), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        let t = nice_ticks(95.3, 105.7, 3);
        assert!(t.contains(&100.0), "{t:?} should include the round 100");
    }

    #[test]
    fn columns_average_their_points() {
        // Four points, two dot columns: each column is the mean of its pair.
        let pts = vec![(0i64, 1.0), (1, 3.0), (2, 10.0), (3, 20.0)];
        assert_eq!(columnise(&pts, 0, 3, 2), vec![(0, 2.0), (1, 15.0)]);
    }

    #[test]
    fn ansi_is_stripped_for_width() {
        assert_eq!(strip_ansi("\x1b[38;5;33mBTC\x1b[39m"), "BTC");
    }
}
