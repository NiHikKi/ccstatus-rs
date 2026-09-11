//! `ccstatus --hook`: called from Claude Code hooks (UserPromptSubmit and
//! PreToolUse for the Skill tool). Counts prompts and skill invocations per
//! session in a small JSON file so the `skills` and `prompts` widgets can show them.
//!
//! This path must never print to stdout (UserPromptSubmit stdout is injected into
//! the model's context) and never exit non-zero (exit 2 would block the prompt).

use crate::input::HookInput;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct SessionState {
    pub prompts: u64,
    pub skills_total: u64,
    pub last_skill: Option<String>,
    pub unique_skills: Vec<String>,
    pub updated: u64,
}

pub fn run(stdin_json: &str) {
    let Ok(h) = serde_json::from_str::<HookInput>(stdin_json) else {
        return;
    };
    let Some(sid) = h.session_id.as_deref() else {
        return;
    };
    let mut st = load(sid).unwrap_or_default();
    match h.hook_event_name.as_deref() {
        Some("UserPromptSubmit") => st.prompts += 1,
        Some("PreToolUse") if h.tool_name.as_deref() == Some("Skill") => {
            st.skills_total += 1;
            let name = h
                .tool_input
                .as_ref()
                .and_then(|v| v.get("skill_name").or_else(|| v.get("skill")).or_else(|| v.get("name")))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if let Some(n) = name {
                if !st.unique_skills.iter().any(|s| s == &n) {
                    st.unique_skills.push(n.clone());
                }
                st.last_skill = Some(n);
            }
        }
        _ => return,
    }
    st.updated = now();
    let _ = save(sid, &st);
}

pub fn load(session_id: &str) -> Option<SessionState> {
    let s = std::fs::read_to_string(state_path(session_id)?).ok()?;
    serde_json::from_str(&s).ok()
}

fn save(session_id: &str, st: &SessionState) -> Option<()> {
    let path = state_path(session_id)?;
    std::fs::create_dir_all(path.parent()?).ok()?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_string(st).ok()?).ok()?;
    std::fs::rename(&tmp, &path).ok()
}

fn state_path(session_id: &str) -> Option<PathBuf> {
    let safe: String = session_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(80)
        .collect();
    if safe.is_empty() {
        return None;
    }
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("ccstatus").join("sessions").join(format!("{safe}.json")))
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
    fn counts_prompts_and_skills_per_session() {
        let dir = std::env::temp_dir().join(format!("ccstatus-test-{}", std::process::id()));
        // SAFETY: tests in this module run single-threaded with respect to this variable.
        unsafe { std::env::set_var("XDG_CACHE_HOME", &dir) };
        let sid = "test-session";
        run(&format!(r#"{{"session_id":"{sid}","hook_event_name":"UserPromptSubmit","prompt":"hi"}}"#));
        run(&format!(
            r#"{{"session_id":"{sid}","hook_event_name":"PreToolUse","tool_name":"Skill","tool_input":{{"skill_name":"deploy"}}}}"#
        ));
        run(&format!(
            r#"{{"session_id":"{sid}","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{{}}}}"#
        ));
        let st = load(sid).expect("state saved");
        assert_eq!(st.prompts, 1);
        assert_eq!(st.skills_total, 1);
        assert_eq!(st.last_skill.as_deref(), Some("deploy"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn garbage_input_is_ignored() {
        run("not json");
        run(r#"{"hook_event_name":"UserPromptSubmit"}"#);
    }
}
