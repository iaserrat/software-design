//! Best-effort persistence for hook sessions.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Session state older than this belongs to sessions that have ended.
const SESSION_TTL: std::time::Duration = std::time::Duration::from_hours(720);

/// What a session remembers between hooks. A missing or unreadable file is the
/// default: no base commit, nothing reported yet.
#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct State {
    /// `HEAD` when the session started; lines added since are the session's.
    #[serde(default)]
    pub(super) base: Option<String>,
    /// Keys of findings already reported, so each is reported once.
    #[serde(default)]
    pub(super) reported: BTreeSet<String>,
}

/// Where a session's [`State`] lives in the plugin's data directory. Absent when
/// the plugin has no data directory.
pub(super) struct Session {
    path: PathBuf,
}

impl Session {
    fn dir() -> Option<PathBuf> {
        Some(PathBuf::from(std::env::var_os("CLAUDE_PLUGIN_DATA")?).join("sessions"))
    }

    pub(super) fn open(session_id: Option<&str>) -> Option<Session> {
        let dir = Session::dir()?;
        let id = session_id.filter(|id| {
            !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })?;
        std::fs::create_dir_all(&dir).ok()?;
        Some(Session {
            path: dir.join(format!("{id}.json")),
        })
    }

    pub(super) fn load(&self) -> State {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Deletes state files of sessions inactive for longer than [`SESSION_TTL`].
    pub(super) fn prune() {
        let Some(entries) = Session::dir().and_then(|d| std::fs::read_dir(d).ok()) else {
            return;
        };
        for entry in entries.flatten() {
            let stale = entry
                .metadata()
                .and_then(|m| m.modified())
                .is_ok_and(|t| t.elapsed().is_ok_and(|age| age > SESSION_TTL));
            if stale {
                // software-design-allow: ignored-error a file another session already pruned is the goal reached
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// Writes through a temporary file so a concurrent reader never sees half a file.
    pub(super) fn save(&self, state: &State) {
        let temp = self.path.with_extension("tmp");
        let Ok(text) = serde_json::to_string(state) else {
            return;
        };
        if std::fs::write(&temp, text).is_ok() {
            // software-design-allow: ignored-error losing state only means a finding is reported twice
            let _ = std::fs::rename(&temp, &self.path);
        }
    }
}
