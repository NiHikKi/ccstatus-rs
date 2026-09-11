//! ccstatus: a fast status line for Claude Code.
//!
//! * `ccstatus`          read the statusLine JSON from stdin, print the lines
//! * `ccstatus --hook`   record a UserPromptSubmit / Skill event (called from hooks)
//! * `ccstatus --init`   write the default config if none exists
//! * `ccstatus --check`  render a sample and report config problems

mod config;
mod git;
mod hook;
mod input;
mod render;
mod widgets;

use std::io::{Read, Write};

use config::Config;
use input::Input;
use render::Segment;

const SAMPLE: &str = include_str!("../examples/sample.json");

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--hook") => hook::run(&read_stdin()),
        Some("--init") => match config::init() {
            Ok((p, true)) => println!("wrote {}", p.display()),
            Ok((p, false)) => println!("exists {}", p.display()),
            Err(e) => {
                eprintln!("ccstatus: cannot write config: {e}");
                std::process::exit(1);
            }
        },
        Some("--check") => check(),
        Some("--version" | "-V") => println!("ccstatus {}", env!("CARGO_PKG_VERSION")),
        Some("--help" | "-h") => print!("{USAGE}"),
        Some(other) => {
            eprintln!("ccstatus: unknown argument {other:?}\n{USAGE}");
            std::process::exit(2);
        }
        None => {
            let (cfg, _) = config::load();
            let input = Input::parse(&read_stdin());
            print_lines(&input, &cfg);
        }
    }
}

const USAGE: &str = "usage: ccstatus [--hook | --init | --check | --version]
  (no args)  render the status line from the JSON on stdin
  --hook     record a hook event from the JSON on stdin (never prints)
  --init     write the default config to ~/.config/ccstatus/config.toml
  --check    render a built-in sample and list config problems
";

fn read_stdin() -> String {
    let mut s = String::new();
    let _ = std::io::stdin().read_to_string(&mut s);
    s
}

fn print_lines(input: &Input, cfg: &Config) {
    let ctx = widgets::Ctx::new(input, cfg);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for names in &cfg.lines {
        let segments = build_line(names, &ctx, cfg);
        if segments.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{}", render::line(&segments, cfg));
    }
}

fn build_line(names: &[String], ctx: &widgets::Ctx, cfg: &Config) -> Vec<Segment> {
    let mut segments = Vec::with_capacity(names.len());
    for name in names {
        let Some(text) = widgets::render(name, ctx) else {
            continue;
        };
        let slot = segments.len();
        let bg = cfg
            .colors
            .get(name.as_str())
            .or_else(|| cfg.colors.get(widgets::canonical(name)))
            .copied()
            .or_else(|| cfg.theme.get(slot % cfg.theme.len().max(1)).copied())
            .unwrap_or(238);
        segments.push(Segment { text, bg });
    }
    segments
}

fn check() {
    let (cfg, err) = config::load();
    println!("config: {}", config::path().display());
    if let Some(e) = err {
        println!("config error, using defaults: {e}");
    }
    let unknown: Vec<&str> = cfg
        .lines
        .iter()
        .flatten()
        .map(|n| widgets::canonical(n))
        .filter(|n| !widgets::KNOWN.contains(n))
        .collect();
    if !unknown.is_empty() {
        println!("unknown widgets (hidden): {}", unknown.join(", "));
    }
    let mut input = Input::parse(SAMPLE);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    if let Some(rl) = input.rate_limits.as_mut() {
        if let Some(l) = rl.five_hour.as_mut() {
            l.resets_at = Some(now + 2.0 * 3600.0 + 15.0 * 60.0);
        }
        if let Some(l) = rl.seven_day.as_mut() {
            l.resets_at = Some(now + 3.0 * 86400.0 + 4.0 * 3600.0);
        }
    }
    if let Some(pc) = input.prompt_cache.as_mut() {
        pc.expires_at = Some(now + 50.0 * 60.0);
    }
    if input.cwd.is_none() {
        input.cwd = std::env::current_dir().ok().map(|p| p.display().to_string());
    }
    println!("sample render:");
    print_lines(&input, &cfg);
    println!("known widgets: {}", widgets::KNOWN.join(", "));
}
