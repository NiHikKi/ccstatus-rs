//! The JSON Claude Code writes to the status line command's stdin.
//!
//! Every field is optional: older Claude Code versions omit some of them, and the
//! status line must never fail to render because of a missing key.
//! Reference: https://code.claude.com/docs/en/statusline

use serde::Deserialize;

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Input {
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    pub cwd: Option<String>,
    pub version: Option<String>,
    pub model: Option<Model>,
    pub workspace: Option<Workspace>,
    pub cost: Option<Cost>,
    pub context_window: Option<ContextWindow>,
    pub exceeds_200k_tokens: Option<bool>,
    pub fast_mode: Option<bool>,
    pub effort: Option<Effort>,
    pub thinking: Option<Thinking>,
    pub rate_limits: Option<RateLimits>,
    pub prompt_cache: Option<PromptCache>,
    pub output_style: Option<Named>,
    pub agent: Option<Named>,
    pub vim: Option<Vim>,
    pub pr: Option<Pr>,
    pub worktree: Option<Worktree>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Model {
    pub id: Option<String>,
    pub display_name: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Workspace {
    pub current_dir: Option<String>,
    pub project_dir: Option<String>,
    pub git_worktree: Option<String>,
    pub added_dirs: Vec<String>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Cost {
    pub total_cost_usd: Option<f64>,
    pub total_duration_ms: Option<f64>,
    pub total_api_duration_ms: Option<f64>,
    pub total_lines_added: Option<f64>,
    pub total_lines_removed: Option<f64>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct ContextWindow {
    pub total_input_tokens: Option<f64>,
    pub total_output_tokens: Option<f64>,
    pub context_window_size: Option<f64>,
    pub used_percentage: Option<f64>,
    pub remaining_percentage: Option<f64>,
    pub current_usage: Option<UsageField>,
}

/// `current_usage` has been both a bare number and an object in the wild.
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum UsageField {
    Number(f64),
    Usage(Usage),
}

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Usage {
    pub input_tokens: Option<f64>,
    pub output_tokens: Option<f64>,
    pub cache_creation_input_tokens: Option<f64>,
    pub cache_read_input_tokens: Option<f64>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Effort {
    pub level: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Thinking {
    pub enabled: Option<bool>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct RateLimits {
    pub five_hour: Option<Limit>,
    pub seven_day: Option<Limit>,
    pub spend_limit: Option<Limit>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Limit {
    pub used_percentage: Option<f64>,
    /// Unix epoch seconds.
    pub resets_at: Option<f64>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct PromptCache {
    pub warm: Option<bool>,
    pub caching_observed: Option<bool>,
    pub ttl: Option<String>,
    /// Unix epoch seconds; null when the last response reported no cache tokens.
    pub expires_at: Option<f64>,
    pub requests: Option<f64>,
    pub misses: Option<f64>,
    /// 0..=1, null until any input tokens were counted.
    pub hit_ratio: Option<f64>,
    pub cache_write_tokens: Option<f64>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Named {
    pub name: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Vim {
    pub mode: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Pr {
    pub number: Option<f64>,
    pub url: Option<String>,
    pub review_state: Option<String>,
    pub kind: Option<String>,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Worktree {
    pub name: Option<String>,
    pub path: Option<String>,
    pub branch: Option<String>,
}

/// Stdin of a hook invocation (`ccstatus --hook`).
/// Reference: https://code.claude.com/docs/en/hooks
#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct HookInput {
    pub session_id: Option<String>,
    pub hook_event_name: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
}

impl Input {
    pub fn parse(json: &str) -> Input {
        serde_json::from_str(json).unwrap_or_default()
    }

    pub fn current_dir(&self) -> Option<&str> {
        self.workspace
            .as_ref()
            .and_then(|w| w.current_dir.as_deref())
            .or(self.cwd.as_deref())
    }

    /// Token counts from the last API call, when reported as an object.
    pub fn usage(&self) -> Option<&Usage> {
        match self.context_window.as_ref()?.current_usage.as_ref()? {
            UsageField::Usage(u) => Some(u),
            UsageField::Number(_) => None,
        }
    }

    /// Tokens currently occupying the context window.
    pub fn context_tokens(&self) -> Option<f64> {
        let cw = self.context_window.as_ref()?;
        match cw.current_usage.as_ref() {
            Some(UsageField::Number(n)) => Some(*n),
            Some(UsageField::Usage(u)) => Some(
                u.input_tokens.unwrap_or(0.0)
                    + u.cache_creation_input_tokens.unwrap_or(0.0)
                    + u.cache_read_input_tokens.unwrap_or(0.0),
            ),
            None => cw.total_input_tokens,
        }
    }

    /// Percentage of the context window in use, 0..=100.
    pub fn context_used_pct(&self) -> Option<f64> {
        let cw = self.context_window.as_ref()?;
        if let Some(p) = cw.used_percentage {
            return Some(p);
        }
        let size = cw.context_window_size.filter(|s| *s > 0.0)?;
        Some(self.context_tokens()? / size * 100.0)
    }
}
