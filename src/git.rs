//! Git branch and working-tree diff, cached on disk so that a status line
//! refresh does not spawn `git` more than once per TTL for the same directory.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub files: u64,
    pub insertions: u64,
    pub deletions: u64,
}

#[derive(Serialize, Deserialize)]
struct Cache {
    ts: u64,
    repo: bool,
    #[serde(default)]
    info: GitInfo,
}

pub fn info(cwd: &str, ttl_secs: u64) -> Option<GitInfo> {
    let now = now();
    let path = cache_path(cwd);
    if let Ok(s) = std::fs::read_to_string(&path)
        && let Ok(c) = serde_json::from_str::<Cache>(&s)
        && now.saturating_sub(c.ts) <= ttl_secs
    {
        return if c.repo { Some(c.info) } else { None };
    }
    let fresh = probe(cwd);
    let entry = Cache {
        ts: now,
        repo: fresh.is_some(),
        info: fresh.clone().unwrap_or_default(),
    };
    if let Ok(s) = serde_json::to_string(&entry) {
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, s).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
    fresh
}

fn probe(cwd: &str) -> Option<GitInfo> {
    let branch = run(cwd, &["symbolic-ref", "--short", "HEAD"]).or_else(|| {
        run(cwd, &["rev-parse", "--short", "HEAD"]).map(|h| format!("detached@{h}"))
    })?;
    let mut info = GitInfo {
        branch: Some(branch),
        ..Default::default()
    };
    if let Some(stat) = run(cwd, &["diff", "--shortstat", "HEAD"]) {
        parse_shortstat(&stat, &mut info);
    }
    Some(info)
}

fn run(cwd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Parses " 3 files changed, 10 insertions(+), 2 deletions(-)".
fn parse_shortstat(stat: &str, info: &mut GitInfo) {
    for part in stat.split(',') {
        let part = part.trim();
        let n: u64 = part
            .split_whitespace()
            .next()
            .and_then(|w| w.parse().ok())
            .unwrap_or(0);
        if part.contains("file") {
            info.files = n;
        } else if part.contains("insertion") {
            info.insertions = n;
        } else if part.contains("deletion") {
            info.deletions = n;
        }
    }
}

fn cache_path(cwd: &str) -> PathBuf {
    std::env::temp_dir().join(format!("ccstatus-git-{:016x}.json", fnv1a(cwd)))
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortstat_parses_all_three_counts() {
        let mut g = GitInfo::default();
        parse_shortstat(" 3 files changed, 10 insertions(+), 2 deletions(-)", &mut g);
        assert_eq!((g.files, g.insertions, g.deletions), (3, 10, 2));
    }

    #[test]
    fn shortstat_handles_missing_parts() {
        let mut g = GitInfo::default();
        parse_shortstat(" 1 file changed, 4 deletions(-)", &mut g);
        assert_eq!((g.files, g.insertions, g.deletions), (1, 0, 4));
    }
}
