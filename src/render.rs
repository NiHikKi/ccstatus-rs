//! Powerline rendering with xterm-256 colors.

use crate::config::Config;

pub struct Segment {
    pub text: String,
    pub bg: u8,
}

/// Renders one status line. Trailing segments are dropped when `max_width` (or
/// the `CCSTATUS_WIDTH` environment variable) says the line would not fit.
pub fn line(segments: &[Segment], cfg: &Config) -> String {
    let limit = std::env::var("CCSTATUS_WIDTH")
        .ok()
        .and_then(|w| w.parse::<usize>().ok())
        .unwrap_or(cfg.max_width);
    let mut n = segments.len();
    if limit > 0 {
        while n > 1 && plain_width(&segments[..n], cfg) > limit {
            n -= 1;
        }
    }
    let segs = &segments[..n];
    let mut out = String::new();
    let Some(first) = segs.first() else {
        return out;
    };
    if !cfg.start_cap.is_empty() {
        out.push_str(&format!("\x1b[38;5;{}m{}", first.bg, cfg.start_cap));
    }
    for (i, s) in segs.iter().enumerate() {
        out.push_str(&format!(
            "\x1b[48;5;{}m\x1b[38;5;{}m {} ",
            s.bg,
            fg_for(s.bg, cfg),
            s.text
        ));
        match segs.get(i + 1) {
            Some(next) => out.push_str(&format!(
                "\x1b[48;5;{}m\x1b[38;5;{}m{}",
                next.bg, s.bg, cfg.separator
            )),
            None => {
                let cap = if cfg.end_cap.is_empty() { &cfg.separator } else { &cfg.end_cap };
                out.push_str(&format!("\x1b[49m\x1b[38;5;{}m{}", s.bg, cap));
            }
        }
    }
    out.push_str("\x1b[0m");
    out
}

/// Width in characters without escape codes: text plus padding and separators.
pub fn plain_width(segs: &[Segment], cfg: &Config) -> usize {
    let sep = cfg.separator.chars().count();
    segs.iter().map(|s| s.text.chars().count() + 2 + sep).sum::<usize>()
        + cfg.start_cap.chars().count()
}

/// Picks a readable foreground for a given xterm-256 background.
pub fn fg_for(bg: u8, cfg: &Config) -> u8 {
    let (r, g, b) = xterm_rgb(bg);
    let lum = (0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64) / 255.0;
    if lum > 0.55 { cfg.fg_dark } else { cfg.fg_light }
}

pub fn xterm_rgb(i: u8) -> (u8, u8, u8) {
    const BASIC: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match i {
        0..=15 => BASIC[i as usize],
        16..=231 => {
            let n = i - 16;
            let level = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
            (level(n / 36), level((n % 36) / 6), level(n % 6))
        }
        232..=255 => {
            let v = 8 + (i - 232) * 10;
            (v, v, v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(t: &str, bg: u8) -> Segment {
        Segment { text: t.to_string(), bg }
    }

    #[test]
    fn cube_and_gray_ramp_map_to_rgb() {
        assert_eq!(xterm_rgb(16), (0, 0, 0));
        assert_eq!(xterm_rgb(231), (255, 255, 255));
        assert_eq!(xterm_rgb(196), (255, 0, 0));
        assert_eq!(xterm_rgb(232), (8, 8, 8));
        assert_eq!(xterm_rgb(255), (238, 238, 238));
    }

    #[test]
    fn light_backgrounds_get_dark_text() {
        let cfg = Config::default();
        assert_eq!(fg_for(222, &cfg), cfg.fg_dark);
        assert_eq!(fg_for(67, &cfg), cfg.fg_light);
    }

    #[test]
    fn separators_carry_previous_background_as_foreground() {
        let cfg = Config { separator: ">".into(), ..Config::default() };
        let out = line(&[seg("a", 1), seg("b", 2)], &cfg);
        assert!(out.contains("\x1b[48;5;2m\x1b[38;5;1m>"), "{out:?}");
        assert!(out.ends_with("\x1b[0m"));
    }

    #[test]
    fn width_limit_drops_trailing_segments() {
        // Each segment is 2 chars of text + 2 padding + 1 separator = 5 columns.
        let cfg = Config { separator: ">".into(), max_width: 10, ..Config::default() };
        let out = line(&[seg("aa", 1), seg("bb", 2), seg("cc", 3)], &cfg);
        assert!(out.contains(" aa "));
        assert!(out.contains(" bb "));
        assert!(!out.contains(" cc "));
    }
}
