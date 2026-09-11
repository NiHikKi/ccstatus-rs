# ccstatus

Rust status line for Claude Code. Reads the `statusLine` stdin JSON, prints
powerline lines. Also `--hook` for hook events, `--init`, `--check`.

## Commands
- `cargo test` runs unit tests (formatting, config, git parsing, hook state).
- `cargo build --release && target/release/ccstatus --check` renders the built-in sample.
- `cargo install --path .` installs to `~/.cargo/bin/ccstatus`.

## Layout
- `src/input.rs`: serde structs for the stdin JSON. Every field is `Option`; never let a missing key break rendering.
- `src/widgets.rs`: one `match` arm per widget, `KNOWN` list, ccstatusline aliases. A widget returns `None` to hide itself.
- `src/render.rs`: powerline segments, xterm-256 foreground choice, width limit.
- `src/config.rs`: TOML config, `DEFAULT_TOML` must stay in sync with `Config::default()` (a test checks this).
- `src/git.rs`: `git symbolic-ref` / `diff --shortstat HEAD` with a per-directory cache file in `$TMPDIR`.
- `src/hook.rs`: per-session counters in `~/.cache/ccstatus/sessions/`.

## Rules
- Render path must stay fast: no network, no Keychain, no transcript parsing. Read only stdin plus small local files.
- `--hook` must never write to stdout and must always exit 0: UserPromptSubmit stdout is injected into the model's context and exit 2 blocks the prompt.
- The stdin contract is documented at https://code.claude.com/docs/en/statusline and https://code.claude.com/docs/en/hooks. Check there before adding fields.
- No new dependencies without a reason; startup time is the product.
