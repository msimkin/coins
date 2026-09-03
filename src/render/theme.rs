//! Colour and the width-tracking line buffer everything else is built from.
//!
//! The series palette is the validated categorical set from the data-viz
//! reference, in its documented order, with two deliberate omissions: slot 6
//! (green) and slot 8 (red) are held back so green and red mean *direction*
//! here and nothing else. Dropping them introduces no new adjacency — the worst
//! adjacent pair is unchanged at CVD ΔE 8.4 dark / 9.1 light (target ≥ 8), and
//! the worst normal-vision pair at 19.3 / 19.6 (floor ≥ 15).
//!
//! Colour is assigned by the coin's position in the config, never by its row in
//! the table, so sorting or filtering never repaints a coin the reader has
//! already learned.

use std::io::IsTerminal;

use crate::config::Theme as ThemeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

const SERIES_DARK: [Rgb; 6] = [
    Rgb(0x39, 0x87, 0xe5), // blue
    Rgb(0xd9, 0x59, 0x26), // orange
    Rgb(0x19, 0x9e, 0x70), // aqua
    Rgb(0xc9, 0x85, 0x00), // yellow
    Rgb(0xd5, 0x51, 0x81), // magenta
    Rgb(0x90, 0x85, 0xe9), // violet
];
const SERIES_LIGHT: [Rgb; 6] = [
    Rgb(0x2a, 0x78, 0xd6),
    Rgb(0xeb, 0x68, 0x34),
    Rgb(0x1b, 0xaf, 0x7a),
    Rgb(0xed, 0xa1, 0x00),
    Rgb(0xe8, 0x7b, 0xa4),
    Rgb(0x4a, 0x3a, 0xa7),
];

const UP_DARK: Rgb = Rgb(0x0c, 0xa3, 0x0c);
const UP_LIGHT: Rgb = Rgb(0x00, 0x63, 0x00);
const DOWN: Rgb = Rgb(0xd0, 0x3b, 0x3b);
const MUTED: Rgb = Rgb(0x89, 0x87, 0x81);
const AXIS_DARK: Rgb = Rgb(0x38, 0x38, 0x35);
const AXIS_LIGHT: Rgb = Rgb(0xc3, 0xc2, 0xb7);

/// How much colour the terminal can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorLevel {
    None,
    Ansi16,
    Xterm256,
    TrueColor,
}

impl ColorLevel {
    pub fn detect() -> ColorLevel {
        // NO_COLOR is honoured whatever its value (https://no-color.org).
        if std::env::var_os("NO_COLOR").is_some() {
            return ColorLevel::None;
        }
        let forced = std::env::var_os("CLICOLOR_FORCE").is_some()
            || std::env::var_os("FORCE_COLOR").is_some();
        if !forced && !std::io::stdout().is_terminal() {
            return ColorLevel::None;
        }
        let term = std::env::var("TERM").unwrap_or_default();
        if term == "dumb" {
            return ColorLevel::None;
        }
        let colorterm = std::env::var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
        if colorterm.contains("truecolor") || colorterm.contains("24bit") {
            return ColorLevel::TrueColor;
        }
        if term.contains("256") {
            return ColorLevel::Xterm256;
        }
        // No TERM at all means we know nothing — unless colour was demanded.
        if term.is_empty() && !forced {
            return ColorLevel::None;
        }
        ColorLevel::Ansi16
    }
}

pub struct Theme {
    pub level: ColorLevel,
    mode: ThemeMode,
}

impl Theme {
    pub fn new(mode: ThemeMode, level: ColorLevel) -> Theme {
        Theme { level, mode }
    }

    /// The colour of the `n`th tracked coin. Past the palette we stop rather
    /// than cycling — a repeated hue would claim two coins are the same one.
    pub fn series(&self, n: usize) -> Rgb {
        let p = match self.mode {
            ThemeMode::Dark => &SERIES_DARK,
            ThemeMode::Light => &SERIES_LIGHT,
        };
        p[n.min(p.len() - 1)]
    }

    pub fn up(&self) -> Rgb {
        match self.mode {
            ThemeMode::Dark => UP_DARK,
            ThemeMode::Light => UP_LIGHT,
        }
    }

    pub fn down(&self) -> Rgb {
        DOWN
    }

    pub fn axis(&self) -> Rgb {
        match self.mode {
            ThemeMode::Dark => AXIS_DARK,
            ThemeMode::Light => AXIS_LIGHT,
        }
    }

    pub fn paint(&self, s: &str, c: Rgb) -> String {
        match self.level {
            ColorLevel::None => s.to_string(),
            ColorLevel::TrueColor => format!("\x1b[38;2;{};{};{}m{s}\x1b[39m", c.0, c.1, c.2),
            ColorLevel::Xterm256 => format!("\x1b[38;5;{}m{s}\x1b[39m", xterm256(c)),
            ColorLevel::Ansi16 => {
                let (code, bright) = ansi16(c);
                let base = if bright { 90 } else { 30 };
                format!("\x1b[{}m{s}\x1b[39m", base + code)
            }
        }
    }

    pub fn dim(&self, s: &str) -> String {
        match self.level {
            ColorLevel::None => s.to_string(),
            _ => self.paint(s, MUTED),
        }
    }

    pub fn bold(&self, s: &str) -> String {
        match self.level {
            ColorLevel::None => s.to_string(),
            _ => format!("\x1b[1m{s}\x1b[22m"),
        }
    }

    /// Green for a rise, red for a fall — always alongside a ▲/▼ glyph, so the
    /// sign never rests on colour alone.
    pub fn delta(&self, s: &str, value: f64) -> String {
        if value > 0.0 {
            self.paint(s, self.up())
        } else if value < 0.0 {
            self.paint(s, self.down())
        } else {
            self.dim(s)
        }
    }
}

/// The xterm 6×6×6 cube plus its 24 greys, picked by nearest squared distance.
fn xterm256(c: Rgb) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let mut best = (u32::MAX, 16u8);
    for (r, &rv) in LEVELS.iter().enumerate() {
        for (g, &gv) in LEVELS.iter().enumerate() {
            for (b, &bv) in LEVELS.iter().enumerate() {
                let d = dist(c, Rgb(rv, gv, bv));
                if d < best.0 {
                    best = (d, 16 + 36 * r as u8 + 6 * g as u8 + b as u8);
                }
            }
        }
    }
    for i in 0..24u8 {
        let v = 8 + 10 * i;
        let d = dist(c, Rgb(v, v, v));
        if d < best.0 {
            best = (d, 232 + i);
        }
    }
    best.1
}

/// Nearest of the 16 standard colours: returns (0-7, bright).
fn ansi16(c: Rgb) -> (u8, bool) {
    const BASIC: [Rgb; 8] = [
        Rgb(0, 0, 0),
        Rgb(0xcd, 0, 0),
        Rgb(0, 0xcd, 0),
        Rgb(0xcd, 0xcd, 0),
        Rgb(0, 0, 0xee),
        Rgb(0xcd, 0, 0xcd),
        Rgb(0, 0xcd, 0xcd),
        Rgb(0xe5, 0xe5, 0xe5),
    ];
    const BRIGHT: [Rgb; 8] = [
        Rgb(0x7f, 0x7f, 0x7f),
        Rgb(0xff, 0, 0),
        Rgb(0, 0xff, 0),
        Rgb(0xff, 0xff, 0),
        Rgb(0x5c, 0x5c, 0xff),
        Rgb(0xff, 0, 0xff),
        Rgb(0, 0xff, 0xff),
        Rgb(0xff, 0xff, 0xff),
    ];
    let mut best = (u32::MAX, 0u8, false);
    for (i, cand) in BASIC.iter().enumerate() {
        let d = dist(c, *cand);
        if d < best.0 {
            best = (d, i as u8, false);
        }
    }
    for (i, cand) in BRIGHT.iter().enumerate() {
        let d = dist(c, *cand);
        if d < best.0 {
            best = (d, i as u8, true);
        }
    }
    (best.1, best.2)
}

fn dist(a: Rgb, b: Rgb) -> u32 {
    let dr = a.0 as i32 - b.0 as i32;
    let dg = a.1 as i32 - b.1 as i32;
    let db = a.2 as i32 - b.2 as i32;
    (dr * dr + dg * dg + db * db) as u32
}

/// A line of output that knows its own visible width, so escape sequences can't
/// throw off padding and columns.
#[derive(Debug, Default, Clone)]
pub struct SLine {
    out: String,
    width: usize,
}

impl SLine {
    pub fn new() -> SLine {
        SLine::default()
    }

    pub fn width(&self) -> usize {
        self.width
    }

    /// Plain, uncoloured text.
    pub fn text(&mut self, s: &str) -> &mut SLine {
        self.out.push_str(s);
        self.width += vis_width(s);
        self
    }

    /// `visible` is what the reader sees; `painted` is the same text with escapes.
    pub fn styled(&mut self, visible: &str, painted: String) -> &mut SLine {
        self.out.push_str(&painted);
        self.width += vis_width(visible);
        self
    }

    pub fn spaces(&mut self, n: usize) -> &mut SLine {
        self.text(&" ".repeat(n))
    }

    pub fn pad_to(&mut self, target: usize) -> &mut SLine {
        if self.width < target {
            self.spaces(target - self.width);
        }
        self
    }

    pub fn finish(self) -> String {
        self.out
    }
}

/// Every glyph this tool prints (braille, blocks, box-drawing, ●▲▼…, currency
/// symbols) is single-width, so counting characters is the right measure.
pub fn vis_width(s: &str) -> usize {
    s.chars().count()
}

/// The terminal, in characters. `$COINS_WIDTH` and `$COINS_HEIGHT` stand in
/// where there is no terminal to measure.
pub fn term_size() -> (usize, usize) {
    let rows = std::env::var("COINS_HEIGHT")
        .ok()
        .and_then(|h| h.trim().parse::<usize>().ok())
        .or_else(|| terminal_size::terminal_size().map(|(_, terminal_size::Height(h))| h as usize))
        .filter(|h| *h >= 4)
        .unwrap_or(24);
    (term_width(), rows)
}

pub fn term_width() -> usize {
    // $COINS_WIDTH is the escape hatch for piping into a pager or a file,
    // where there is no terminal to measure.
    if let Some(w) = std::env::var("COINS_WIDTH").ok().and_then(|w| w.trim().parse::<usize>().ok()) {
        if w >= 40 {
            return w;
        }
    }
    terminal_size::terminal_size()
        .map(|(terminal_size::Width(w), _)| w as usize)
        .filter(|w| *w >= 40)
        .unwrap_or(80)
}
