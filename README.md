# ccstatus

A small, fast status line for [Claude Code](https://code.claude.com). It reads the
JSON that Claude Code writes to the `statusLine` command's stdin and prints
powerline-styled lines: model, effort, context usage, git branch and diff, memory,
token and cache statistics, session cost and clock, 5-hour / 7-day rate limits with
reset timers, skill usage.

Written in Rust as a replacement for the Node-based `ccstatusline`: one render takes a
few milliseconds and a few megabytes instead of ~200 ms and ~120 MB for a Node start.
Everything comes from the stdin JSON, so there are no network calls, no Keychain reads
and no transcript parsing.

## Install

```bash
cargo install --path .        # puts `ccstatus` into ~/.cargo/bin
ccstatus --init               # writes ~/.config/ccstatus/config.toml
ccstatus --check              # renders a sample, lists unknown widgets
```

Then in `~/.claude/settings.json`:

```json
{
  "statusLine": { "type": "command", "command": "ccstatus", "padding": 0, "refreshInterval": 60 },
  "hooks": {
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "ccstatus --hook" }] }],
    "PreToolUse": [{ "matcher": "Skill", "hooks": [{ "type": "command", "command": "ccstatus --hook" }] }]
  }
}
```

The hooks are optional; they only feed the `skills` and `prompts` widgets.
`refreshInterval` is only needed for the time-based widgets (`cache-timer`,
`reset-timer`, `session-clock`); without it the line updates on session events.

## Configure

`~/.config/ccstatus/config.toml` (or `$CCSTATUS_CONFIG`):

```toml
separator = ""               # powerline arrow, "" to disable
theme = [131, 173, 222, 144, 139, 110, 109, 67]   # xterm-256 backgrounds, cycled
git_cache_ttl_secs = 5
max_width = 0                      # or set CCSTATUS_WIDTH=<columns>

lines = [
  ["version", "model", "effort", "context-length", "git-branch", "git-changes"],
  ["cwd", "memory", "account-email", "cache-timer"],
  ["tokens-input", "tokens-output", "tokens-cached", "tokens-total", "context-bar", "cache-hit-rate", "cache-read", "cache-write"],
  ["session-clock", "session-cost", "skills", "session-usage", "weekly-usage", "reset-timer", "weekly-reset-timer"],
]

[colors]
"git-branch" = 5                   # fixed background for one widget
```

Widgets that have nothing to show (no git repo, no rate-limit data, no cache yet)
are hidden rather than rendered empty. ccstatusline names such as
`current-working-dir`, `free-memory`, `thinking-effort` and
`claude-account-email` are accepted as aliases.

| Widget | Shows | Source |
|---|---|---|
| `version`, `model`, `effort`, `vim`, `agent`, `output-style`, `session-name` | as named | stdin |
| `context-length`, `context-pct`, `context-bar`, `over-200k` | tokens in the window, % used | `context_window` |
| `tokens-input`, `tokens-output`, `tokens-total` | session totals | `context_window` |
| `tokens-cached`, `cache-read`, `cache-write`, `cache-hit-rate`, `cache-timer` | last call cache tokens, hit ratio, TTL countdown | `current_usage`, `prompt_cache` |
| `session-clock`, `session-cost`, `lines-changed` | wall time, USD, +/- lines | `cost` |
| `session-usage`, `weekly-usage`, `reset-timer`, `weekly-reset-timer`, `spend-limit` | 5h / 7d limits and resets | `rate_limits` |
| `git-branch`, `git-changes`, `worktree`, `pr` | branch, `+ins -del` vs HEAD, worktree, PR/MR | `git` (cached), stdin |
| `cwd`, `memory`, `account-email`, `fast-mode` | path, used/total RAM, login email | fs, sysinfo, `.claude.json` |
| `skills`, `prompts` | last skill and count, prompt count | `--hook` state |

Not ported from ccstatusline: input/output token speed, per-model weekly limits,
the interactive configurator. Edit the TOML instead.

## Files it touches

- `~/.config/ccstatus/config.toml`: configuration.
- `$TMPDIR/ccstatus-git-<hash>.json`: git probe cache per directory.
- `~/.cache/ccstatus/sessions/<session_id>.json`: hook counters.

## Develop

```bash
cargo test
cargo build --release && ccstatus --check
echo '{"model":{"display_name":"Fable"}}' | target/release/ccstatus
```

## Licence

MIT, see `LICENSE`. Nothing here is derived from another status line project;
`ATTRIBUTION.md` records that.
