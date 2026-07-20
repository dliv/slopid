use crate::input::CollectedInput;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::load_project_config;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NoteState {
    Pending,
    Quarantined,
    Cancelled,
}

#[derive(Debug, Serialize)]
pub struct NoteResult {
    pub state: NoteState,
    pub path: Option<PathBuf>,
}

pub fn cmd_note(input: CollectedInput, cwd: &Path) -> Result<NoteResult> {
    let CollectedInput::Text(text) = input else {
        return Ok(NoteResult {
            state: NoteState::Cancelled,
            path: None,
        });
    };
    let config = load_project_config(cwd)?;
    write_note_at(
        &config.note_root,
        text.as_bytes(),
        Utc::now(),
        std::iter::repeat_with(|| fastrand::u32(..)),
    )
}

fn write_note_at<I: Iterator<Item = u32>>(
    note_root: &Path,
    text: &[u8],
    now: DateTime<Utc>,
    mut nonces: I,
) -> Result<NoteResult> {
    let quarantined = contains_suspected_secret(std::str::from_utf8(text).unwrap_or_default());
    let root = if quarantined {
        note_root.join("quarantine")
    } else {
        note_root.to_path_buf()
    };
    fs::create_dir_all(&root)
        .with_context(|| format!("create capture note directory {}", root.display()))?;
    let prefix = now.format("%Y%m%dT%H%M%S%.6fZ").to_string();
    for _ in 0..100 {
        let nonce = nonces
            .next()
            .ok_or_else(|| anyhow::anyhow!("note nonce source exhausted"))?;
        let path = root.join(format!("{prefix}_{nonce:08x}.md"));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| format!("create capture note {}", path.display()));
            }
        };
        file.write_all(text)
            .with_context(|| format!("write capture note {}", path.display()))?;
        file.sync_all()
            .with_context(|| format!("sync capture note {}", path.display()))?;
        return Ok(NoteResult {
            state: if quarantined {
                NoteState::Quarantined
            } else {
                NoteState::Pending
            },
            path: Some(path),
        });
    }
    bail!("cannot allocate an exclusive capture note filename after 100 attempts")
}

fn contains_suspected_secret(text: &str) -> bool {
    static TOKEN_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    static ASSIGNMENT: OnceLock<Regex> = OnceLock::new();
    let token_patterns = TOKEN_PATTERNS.get_or_init(|| {
        [
            r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
            r"AKIA[0-9A-Z]{16}",
            r"gh[pousr]_[A-Za-z0-9]{20,}",
            r"sk-[A-Za-z0-9_-]{20,}",
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("fixed secret regex"))
        .collect()
    });
    if token_patterns.iter().any(|pattern| pattern.is_match(text)) {
        return true;
    }
    let assignment = ASSIGNMENT.get_or_init(|| {
        Regex::new(
            r#"(?im)^\s*\{?\s*["']?([A-Za-z][A-Za-z0-9_.-]*)["']?\s*[:=]\s*(.+?)\s*[,}]?\s*$"#,
        )
        .expect("fixed assignment regex")
    });
    let recognized = [
        "password",
        "passwd",
        "clientsecret",
        "apikey",
        "accesskey",
        "secretkey",
        "keystorepassword",
        "truststorepassword",
        "keyvaultkey",
    ];
    assignment.captures_iter(text).any(|capture| {
        let key = capture[1]
            .chars()
            .filter(|ch| !matches!(ch, '_' | '-' | '.'))
            .flat_map(char::to_lowercase)
            .collect::<String>();
        if !recognized.contains(&key.as_str()) {
            return false;
        }
        let value = capture[2]
            .trim()
            .trim_matches(|ch| matches!(ch, '"' | '\''))
            .trim();
        if value.is_empty() {
            return false;
        }
        let lower = value.to_lowercase();
        !(matches!(
            lower.as_str(),
            "null" | "redacted" | "[redacted]" | "changeme" | "example"
        ) || value.starts_with('$') && value.get(1..2) == Some("{")
            || value.starts_with('<') && value.ends_with('>'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};

    #[test]
    fn secret_floor_covers_assignments_keys_and_tokens_without_placeholder_noise() {
        for positive in [
            r#"{"password": "actual-secret"}"#,
            "keyStorePassword = actual-secret",
            "trustStorePassword: actual-secret",
            "keyVaultKey: actual-secret",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
            "AKIA1234567890ABCDEF",
            "ghp_12345678901234567890",
            "sk-12345678901234567890",
        ] {
            assert!(contains_suspected_secret(positive), "{positive}");
        }
        for control in [
            "rotate the password every month",
            "password:",
            concat!("password: $", "{ENV}"),
            "password: <placeholder>",
            "password: [REDACTED]",
            "password: example",
        ] {
            assert!(!contains_suspected_secret(control), "{control}");
        }
    }

    #[test]
    fn injected_clock_and_nonce_lock_filename_shape_and_collision_retry() {
        let tmp = tempfile::tempdir().unwrap();
        let now = Utc
            .with_ymd_and_hms(2026, 7, 13, 21, 30, 40)
            .unwrap()
            .with_nanosecond(123_456_000)
            .unwrap();
        fs::write(
            tmp.path().join("20260713T213040.123456Z_00000001.md"),
            "occupied",
        )
        .unwrap();
        let result = write_note_at(tmp.path(), b"safe", now, [1, 2].into_iter()).unwrap();
        assert!(
            result
                .path
                .unwrap()
                .ends_with("20260713T213040.123456Z_00000002.md")
        );
    }
}
