//! Widgets: each turns the stdin JSON (plus git, memory and hook state) into a
//! short piece of text, or `None` to stay hidden.

use std::cell::OnceCell;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::git::{self, GitInfo};
use crate::hook::{self, SessionState};
use crate::input::Input;

/// Every widget name `render` understands. ccstatusline names are accepted as
/// aliases so an existing configuration can be carried over.
pub const KNOWN: &[&str] = &[
    "version",
    "model",
    "effort",
    "context-length",
    "context-pct",
    "context-bar",
    "git-branch",
    "git-changes",
    "cwd",
    "memory",
    "account-email",
    "cache-timer",
    "tokens-input",
    "tokens-output",
    "tokens-cached",
    "tokens-total",
    "cache-hit-rate",
    "cache-read",
    "cache-write",
    "session-clock",
    "session-cost",
    "lines-changed",
    "skills",
    "prompts",
    "session-usage",
    "weekly-usage",
    "reset-timer",
    "weekly-reset-timer",
    "spend-limit",
    "session-name",
    "worktree",
    "pr",
    "vim",
    "agent",
    "output-style",
    "fast-mode",
    "over-200k",
];

const ALIASES: &[(&str, &str)] = &[
    ("thinking-effort", "effort"),
    ("current-working-dir", "cwd"),
    ("free-memory", "memory"),
    ("claude-account-email", "account-email"),
    ("five-hour-usage", "session-usage"),
    ("seven-day-usage", "weekly-usage"),
];

pub fn canonical(name: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map(|(_, c)| *c)
        .unwrap_or(name)
}

pub struct Ctx<'a> {
    pub input: &'a Input,
    pub cfg: &'a Config,
    pub now: u64,
    git: OnceCell<Option<GitInfo>>,
    state: OnceCell<Option<SessionState>>,
    mem: OnceCell<Option<(u64, u64)>>,
}

impl<'a> Ctx<'a> {
    pub fn new(input: &'a Input, cfg: &'a Config) -> Self {
        Ctx {
            input,
            cfg,
            now: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            git: OnceCell::new(),
            state: OnceCell::new(),
            mem: OnceCell::new(),
        }
    }

    fn git(&self) -> Option<&GitInfo> {
        self.git
            .get_or_init(|| {
                self.input
                    .current_dir()
                    .and_then(|d| git::info(d, self.cfg.git_cache_ttl_secs))
            })
            .as_ref()
    }

    fn state(&self) -> Option<&SessionState> {
        self.state
            .get_or_init(|| self.input.session_id.as_deref().and_then(hook::load))
            .as_ref()
    }

    /// (used, total) bytes.
    fn mem(&self) -> Option<(u64, u64)> {
        *self.mem.get_or_init(|| {
            let mut s = sysinfo::System::new();
            s.refresh_memory();
            let total = s.total_memory();
            (total > 0).then_some((s.used_memory(), total))
        })
    }
}

pub fn render(name: &str, ctx: &Ctx) -> Option<String> {
    let i = ctx.input;
    match canonical(name) {
        "version" => i.version.as_ref().map(|v| format!("v{v}")),
        "model" => i
            .model
            .as_ref()
            .and_then(|m| m.display_name.clone().or_else(|| m.id.clone())),
        "effort" => i.effort.as_ref().and_then(|e| e.level.clone()),
        "context-length" => i.context_tokens().map(|t| format!("{} ctx", tokens(t))),
        "context-pct" => i.context_used_pct().map(|p| format!("{p:.0}%")),
        "context-bar" => i.context_used_pct().map(bar),
        "git-branch" => ctx
            .git()
            .and_then(|g| g.branch.as_ref())
            .map(|b| format!("\u{e0a0} {b}")),
        "git-changes" => ctx.git().and_then(|g| {
            (g.insertions + g.deletions > 0).then(|| format!("+{} -{}", g.insertions, g.deletions))
        }),
        "cwd" => i.current_dir().map(short_path),
        "memory" => ctx
            .mem()
            .map(|(used, total)| format!("Mem {:.1}G/{:.0}G", gib(used), gib(total))),
        "account-email" => account_email(),
        "cache-timer" => {
            let pc = i.prompt_cache.as_ref()?;
            match (pc.warm, pc.expires_at) {
                (Some(true), Some(exp)) => Some(format!("\u{23f1} {}", until(exp, ctx.now))),
                _ if pc.caching_observed == Some(true) => Some("cache cold".to_string()),
                _ => None,
            }
        }
        "tokens-input" => i
            .context_window
            .as_ref()
            .and_then(|c| c.total_input_tokens)
            .map(|t| format!("In {}", tokens(t))),
        "tokens-output" => i
            .context_window
            .as_ref()
            .and_then(|c| c.total_output_tokens)
            .map(|t| format!("Out {}", tokens(t))),
        "tokens-cached" => i
            .usage()
            .and_then(|u| u.cache_read_input_tokens)
            .map(|t| format!("Cached {}", tokens(t))),
        "tokens-total" => {
            let c = i.context_window.as_ref()?;
            let total = c.total_input_tokens? + c.total_output_tokens.unwrap_or(0.0);
            Some(format!("\u{3a3} {}", tokens(total)))
        }
        "cache-hit-rate" => {
            let from_stats = i.prompt_cache.as_ref().and_then(|p| p.hit_ratio);
            let from_usage = i.usage().and_then(|u| {
                let read = u.cache_read_input_tokens.unwrap_or(0.0);
                let all = read + u.cache_creation_input_tokens.unwrap_or(0.0) + u.input_tokens.unwrap_or(0.0);
                (all > 0.0).then(|| read / all)
            });
            from_stats.or(from_usage).map(|r| format!("{:.0}% hit", r * 100.0))
        }
        "cache-read" => i
            .usage()
            .and_then(|u| u.cache_read_input_tokens)
            .map(|t| format!("R {}", tokens(t))),
        "cache-write" => i
            .usage()
            .and_then(|u| u.cache_creation_input_tokens)
            .map(|t| format!("W {}", tokens(t))),
        "session-clock" => i
            .cost
            .as_ref()
            .and_then(|c| c.total_duration_ms)
            .map(|ms| duration((ms / 1000.0) as u64)),
        "session-cost" => i
            .cost
            .as_ref()
            .and_then(|c| c.total_cost_usd)
            .map(|c| format!("${c:.2}")),
        "lines-changed" => {
            let c = i.cost.as_ref()?;
            let (a, r) = (c.total_lines_added.unwrap_or(0.0), c.total_lines_removed.unwrap_or(0.0));
            (a + r > 0.0).then(|| format!("+{a:.0} -{r:.0}"))
        }
        "skills" => ctx.state().and_then(|s| {
            (s.skills_total > 0).then(|| match &s.last_skill {
                Some(last) => format!("\u{26a1} {last} ({})", s.skills_total),
                None => format!("\u{26a1} {}", s.skills_total),
            })
        }),
        "prompts" => ctx
            .state()
            .filter(|s| s.prompts > 0)
            .map(|s| format!("#{}", s.prompts)),
        "session-usage" => i
            .rate_limits
            .as_ref()
            .and_then(|r| r.five_hour.as_ref())
            .and_then(|l| l.used_percentage)
            .map(|p| format!("5h {p:.0}%")),
        "weekly-usage" => i
            .rate_limits
            .as_ref()
            .and_then(|r| r.seven_day.as_ref())
            .and_then(|l| l.used_percentage)
            .map(|p| format!("7d {p:.0}%")),
        "reset-timer" => i
            .rate_limits
            .as_ref()
            .and_then(|r| r.five_hour.as_ref())
            .and_then(|l| l.resets_at)
            .map(|t| format!("\u{21bb} {}", until(t, ctx.now))),
        "weekly-reset-timer" => i
            .rate_limits
            .as_ref()
            .and_then(|r| r.seven_day.as_ref())
            .and_then(|l| l.resets_at)
            .map(|t| format!("\u{21bb} {}", until(t, ctx.now))),
        "spend-limit" => i
            .rate_limits
            .as_ref()
            .and_then(|r| r.spend_limit.as_ref())
            .and_then(|l| l.used_percentage)
            .map(|p| format!("spend {p:.0}%")),
        "session-name" => i.session_name.clone(),
        "worktree" => i
            .workspace
            .as_ref()
            .and_then(|w| w.git_worktree.clone())
            .map(|w| format!("wt:{w}")),
        "pr" => i.pr.as_ref().and_then(|pr| {
            let n = pr.number?;
            let kind = if pr.kind.as_deref() == Some("mr") { "MR" } else { "PR" };
            Some(match &pr.review_state {
                Some(s) => format!("{kind} #{n:.0} {s}"),
                None => format!("{kind} #{n:.0}"),
            })
        }),
        "vim" => i.vim.as_ref().and_then(|v| v.mode.clone()),
        "agent" => i.agent.as_ref().and_then(|a| a.name.clone()),
        "output-style" => i
            .output_style
            .as_ref()
            .and_then(|o| o.name.clone())
            .filter(|n| n != "default"),
        "fast-mode" => (i.fast_mode == Some(true)).then(|| "fast".to_string()),
        "over-200k" => (i.exceeds_200k_tokens == Some(true)).then(|| "\u{26a0} >200k".to_string()),
        _ => None,
    }
}

/// 950 -> "950", 12_345 -> "12.3k", 1_234_567 -> "1.2M".
pub fn tokens(t: f64) -> String {
    if t >= 1e6 {
        format!("{:.1}M", t / 1e6)
    } else if t >= 1e3 {
        format!("{:.1}k", t / 1e3)
    } else {
        format!("{t:.0}")
    }
}

/// Seconds -> "1h23m", "45m", "12s".
pub fn duration(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h{m:02}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

/// Time left until an epoch timestamp: "3d4h", "2h15m", "9m30s", "now".
pub fn until(epoch: f64, now: u64) -> String {
    let left = epoch as i64 - now as i64;
    if left <= 0 {
        return "now".to_string();
    }
    let left = left as u64;
    let (d, h, m, s) = (left / 86400, (left % 86400) / 3600, (left % 3600) / 60, left % 60);
    if d > 0 {
        format!("{d}d{h}h")
    } else if h > 0 {
        format!("{h}h{m:02}m")
    } else {
        format!("{m}m{s:02}s")
    }
}

fn bar(pct: f64) -> String {
    let pct = pct.clamp(0.0, 100.0);
    let filled = (pct / 10.0).round() as usize;
    let mut b = String::with_capacity(16);
    for _ in 0..filled {
        b.push('\u{2588}');
    }
    for _ in filled..10 {
        b.push('\u{2591}');
    }
    format!("{b} {pct:.0}%")
}

fn gib(bytes: u64) -> f64 {
    bytes as f64 / 1_073_741_824.0
}

fn short_path(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && p.starts_with(&home) => format!("~{}", &p[home.len()..]),
        _ => p.to_string(),
    }
}

/// Reads `oauthAccount.emailAddress` from `.claude.json` without parsing the
/// whole (often hundreds of KB) file.
fn account_email() -> Option<String> {
    let path = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(d) => std::path::PathBuf::from(d).join(".claude.json"),
        None => std::path::PathBuf::from(std::env::var_os("HOME")?).join(".claude.json"),
    };
    let s = std::fs::read_to_string(path).ok()?;
    let key = "\"emailAddress\"";
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    let open = rest.find('"')? + 1;
    let close = rest[open..].find('"')?;
    let email = &rest[open..open + close];
    (!email.is_empty()).then(|| email.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_formatting() {
        assert_eq!(tokens(950.0), "950");
        assert_eq!(tokens(12_345.0), "12.3k");
        assert_eq!(tokens(1_234_567.0), "1.2M");
    }

    #[test]
    fn duration_and_until_formatting() {
        assert_eq!(duration(12), "12s");
        assert_eq!(duration(45 * 60), "45m");
        assert_eq!(duration(3600 + 23 * 60), "1h23m");
        assert_eq!(until(100.0, 200), "now");
        assert_eq!(until(1000.0 + 9.0 * 60.0 + 30.0, 1000), "9m30s");
        assert_eq!(until(1000.0 + 2.0 * 3600.0 + 15.0 * 60.0, 1000), "2h15m");
        assert_eq!(until(1000.0 + 3.0 * 86400.0 + 4.0 * 3600.0, 1000), "3d4h");
    }

    #[test]
    fn bar_rounds_to_tenths() {
        assert_eq!(bar(42.0), "\u{2588}\u{2588}\u{2588}\u{2588}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591}\u{2591} 42%");
        assert!(bar(100.0).starts_with(&"\u{2588}".repeat(10)));
    }

    #[test]
    fn aliases_resolve_to_canonical_names() {
        assert_eq!(canonical("free-memory"), "memory");
        assert_eq!(canonical("model"), "model");
    }

    #[test]
    fn widgets_from_sample_input() {
        let json = r#"{
            "version": "2.1.0",
            "model": {"id": "claude-fable-5-1", "display_name": "Fable"},
            "cost": {"total_cost_usd": 1.234, "total_duration_ms": 5025000},
            "context_window": {"total_input_tokens": 118000, "total_output_tokens": 9200,
                "context_window_size": 1000000,
                "current_usage": {"input_tokens": 1800, "output_tokens": 640,
                    "cache_creation_input_tokens": 2100, "cache_read_input_tokens": 114000}},
            "effort": {"level": "xhigh"},
            "rate_limits": {"five_hour": {"used_percentage": 37, "resets_at": 0},
                            "seven_day": {"used_percentage": 61}}
        }"#;
        let input = Input::parse(json);
        let cfg = Config::default();
        let ctx = Ctx::new(&input, &cfg);
        assert_eq!(render("version", &ctx).as_deref(), Some("v2.1.0"));
        assert_eq!(render("model", &ctx).as_deref(), Some("Fable"));
        assert_eq!(render("thinking-effort", &ctx).as_deref(), Some("xhigh"));
        assert_eq!(render("context-length", &ctx).as_deref(), Some("117.9k ctx"));
        assert_eq!(render("context-pct", &ctx).as_deref(), Some("12%"));
        assert_eq!(render("tokens-total", &ctx).as_deref(), Some("\u{3a3} 127.2k"));
        assert_eq!(render("cache-hit-rate", &ctx).as_deref(), Some("97% hit"));
        assert_eq!(render("session-clock", &ctx).as_deref(), Some("1h23m"));
        assert_eq!(render("session-cost", &ctx).as_deref(), Some("$1.23"));
        assert_eq!(render("session-usage", &ctx).as_deref(), Some("5h 37%"));
        assert_eq!(render("weekly-usage", &ctx).as_deref(), Some("7d 61%"));
        assert_eq!(render("reset-timer", &ctx).as_deref(), Some("\u{21bb} now"));
        assert_eq!(render("weekly-reset-timer", &ctx), None);
        assert_eq!(render("vim", &ctx), None);
        assert_eq!(render("no-such-widget", &ctx), None);
    }
}
