//! TOML configuration: which widgets go on which line, colors, powerline glyphs.
//!
//! Location: `$CCSTATUS_CONFIG`, else `$XDG_CONFIG_HOME/ccstatus/config.toml`,
//! else `~/.config/ccstatus/config.toml`. A missing or broken file falls back to
//! the defaults below, so the status line always renders.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    /// Glyph drawn between segments (powerline arrow by default).
    pub separator: String,
    /// Optional glyph before the first segment.
    pub start_cap: String,
    /// Glyph after the last segment; the separator is reused when empty.
    pub end_cap: String,
    /// xterm-256 background colors, cycled across the visible widgets of a line.
    pub theme: Vec<u8>,
    /// Foreground used on light backgrounds.
    pub fg_dark: u8,
    /// Foreground used on dark backgrounds.
    pub fg_light: u8,
    /// How long a git probe result is reused, in seconds.
    pub git_cache_ttl_secs: u64,
    /// Drop trailing widgets so a line fits this many columns; 0 disables.
    pub max_width: usize,
    /// One array of widget names per rendered line.
    pub lines: Vec<Vec<String>>,
    /// Per-widget background override: `"git-branch" = 5`.
    pub colors: HashMap<String, u8>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            separator: "\u{e0b0}".to_string(),
            start_cap: String::new(),
            end_cap: String::new(),
            // Nord "aurora" plus "frost": red, orange, yellow, green, purple, cyan, blue, deep blue.
            theme: vec![131, 173, 222, 144, 139, 110, 109, 67],
            fg_dark: 16,
            fg_light: 231,
            git_cache_ttl_secs: 5,
            max_width: 0,
            lines: vec![
                strs(&["version", "model", "effort", "context-length", "git-branch", "git-changes"]),
                strs(&["cwd", "memory", "account-email", "cache-timer"]),
                strs(&[
                    "tokens-input",
                    "tokens-output",
                    "tokens-cached",
                    "tokens-total",
                    "context-bar",
                    "cache-hit-rate",
                    "cache-read",
                    "cache-write",
                ]),
                strs(&[
                    "session-clock",
                    "session-cost",
                    "skills",
                    "session-usage",
                    "weekly-usage",
                    "reset-timer",
                    "weekly-reset-timer",
                ]),
            ],
            colors: HashMap::new(),
        }
    }
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// Written by `ccstatus --init`; mirrors `Config::default()`.
pub const DEFAULT_TOML: &str = r#"# ccstatus configuration. Widget names are listed by `ccstatus --check`.
# Powerline glyphs need a patched font (Nerd Font / Powerline). Use "" to disable.
# "" is the solid right-pointing powerline arrow.
separator = ""
start_cap = ""
end_cap = ""

# xterm-256 background colors, cycled across the widgets of each line.
theme = [131, 173, 222, 144, 139, 110, 109, 67]
fg_dark = 16
fg_light = 231

# Reuse git branch/diff results for this many seconds.
git_cache_ttl_secs = 5
# Drop trailing widgets so a line fits this many columns (0 = never).
max_width = 0

lines = [
  ["version", "model", "effort", "context-length", "git-branch", "git-changes"],
  ["cwd", "memory", "account-email", "cache-timer"],
  ["tokens-input", "tokens-output", "tokens-cached", "tokens-total", "context-bar", "cache-hit-rate", "cache-read", "cache-write"],
  ["session-clock", "session-cost", "skills", "session-usage", "weekly-usage", "reset-timer", "weekly-reset-timer"],
]

# Fixed background per widget, overriding the theme cycle.
[colors]
# "git-branch" = 5
"#;

pub fn path() -> PathBuf {
    if let Some(p) = std::env::var_os("CCSTATUS_CONFIG") {
        return PathBuf::from(p);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("ccstatus").join("config.toml")
}

/// Loads the config, or the defaults when the file is absent or invalid.
/// The error, if any, is returned so `--check` can show it.
pub fn load() -> (Config, Option<String>) {
    let p = path();
    match std::fs::read_to_string(&p) {
        Ok(s) => match toml::from_str::<Config>(&s) {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some(format!("{}: {e}", p.display()))),
        },
        Err(_) => (Config::default(), None),
    }
}

/// Writes the default config unless one already exists. Returns the path.
pub fn init() -> std::io::Result<(PathBuf, bool)> {
    let p = path();
    if p.exists() {
        return Ok((p, false));
    }
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&p, DEFAULT_TOML)?;
    Ok((p, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_toml_matches_default_config() {
        let parsed: Config = toml::from_str(DEFAULT_TOML).expect("DEFAULT_TOML parses");
        let d = Config::default();
        assert_eq!(parsed.separator, d.separator);
        assert_eq!(parsed.theme, d.theme);
        assert_eq!(parsed.lines, d.lines);
        assert_eq!(parsed.git_cache_ttl_secs, d.git_cache_ttl_secs);
    }

    #[test]
    fn partial_config_keeps_defaults() {
        let c: Config = toml::from_str("lines = [[\"model\"]]").unwrap();
        assert_eq!(c.lines, vec![vec!["model".to_string()]]);
        assert_eq!(c.theme, Config::default().theme);
    }
}
