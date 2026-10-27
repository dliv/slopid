use serde::Serialize;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug)]
pub enum Failure {
    Config(anyhow::Error),
    New(anyhow::Error),
    List(anyhow::Error),
    ResolveInvalid(String),
    ResolveNotFound(String),
    ResolveAmbiguous {
        message: String,
        candidates: Vec<AmbiguousCandidate>,
    },
    ResolveUnavailable(anyhow::Error),
    Init(anyhow::Error),
    Runtime(anyhow::Error),
    Output(anyhow::Error),
    Usage(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct AmbiguousCandidate {
    pub id: String,
    pub path: PathBuf,
}

impl Failure {
    fn code(&self) -> &'static str {
        match self {
            Self::Config(_) => "sid-config",
            Self::New(_) => "sid-new",
            Self::List(_) => "sid-list",
            Self::ResolveInvalid(_) => "sid-resolve-invalid",
            Self::ResolveNotFound(_) => "sid-resolve-not-found",
            Self::ResolveAmbiguous { .. } => "sid-resolve-ambiguous",
            Self::ResolveUnavailable(_) => "sid-resolve-unavailable",
            Self::Init(_) => "sid-init",
            Self::Runtime(_) => "sid-runtime",
            Self::Output(_) => "sid-output",
            Self::Usage(_) => "sid-usage",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Config(error)
            | Self::New(error)
            | Self::List(error)
            | Self::ResolveUnavailable(error)
            | Self::Init(error)
            | Self::Runtime(error)
            | Self::Output(error) => format!("{error:#}"),
            Self::ResolveInvalid(message)
            | Self::ResolveNotFound(message)
            | Self::Usage(message) => message.clone(),
            Self::ResolveAmbiguous { message, .. } => message.clone(),
        }
    }

    fn exit_code(&self) -> i32 {
        if matches!(self, Self::Usage(_)) { 2 } else { 1 }
    }

    fn envelope(&self) -> ErrorEnvelope {
        let candidates = match self {
            Self::ResolveAmbiguous { candidates, .. } => Some(candidates.clone()),
            _ => None,
        };
        ErrorEnvelope {
            error: ErrorBody {
                code: self.code(),
                message: self.message(),
                candidates,
            },
        }
    }
}

#[derive(Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidates: Option<Vec<AmbiguousCandidate>>,
}

pub fn report_failure(failure: Failure) -> ! {
    let code = failure.exit_code();
    let mut stderr = io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &failure.envelope());
    let _ = writeln!(stderr);
    std::process::exit(code)
}

pub fn write_json<T: Serialize>(value: &T) -> Result<(), Failure> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    serde_json::to_writer_pretty(&mut stdout, value)
        .map_err(|error| Failure::Output(error.into()))?;
    writeln!(stdout).map_err(|error| Failure::Output(error.into()))
}

pub fn write_text(value: &str) -> Result<(), Failure> {
    let stdout = io::stdout();
    stdout
        .lock()
        .write_all(value.as_bytes())
        .map_err(|error| Failure::Output(error.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_failure_codes_are_selected_by_variants() {
        let cases = [
            (Failure::Config(anyhow::anyhow!("x")), "sid-config", 1),
            (Failure::New(anyhow::anyhow!("x")), "sid-new", 1),
            (Failure::List(anyhow::anyhow!("x")), "sid-list", 1),
            (
                Failure::ResolveInvalid("x".to_string()),
                "sid-resolve-invalid",
                1,
            ),
            (
                Failure::ResolveNotFound("x".to_string()),
                "sid-resolve-not-found",
                1,
            ),
            (
                Failure::ResolveUnavailable(anyhow::anyhow!("x")),
                "sid-resolve-unavailable",
                1,
            ),
            (Failure::Init(anyhow::anyhow!("x")), "sid-init", 1),
            (Failure::Runtime(anyhow::anyhow!("x")), "sid-runtime", 1),
            (Failure::Output(anyhow::anyhow!("x")), "sid-output", 1),
            (Failure::Usage("x".to_string()), "sid-usage", 2),
        ];
        for (failure, code, exit_code) in cases {
            assert_eq!(failure.code(), code);
            assert_eq!(failure.exit_code(), exit_code);
        }
    }

    #[test]
    fn only_ambiguity_serializes_sorted_candidate_fields() {
        let failure = Failure::ResolveAmbiguous {
            message: "ambiguous".to_string(),
            candidates: vec![AmbiguousCandidate {
                id: "202610_sa2a7_alpha".to_string(),
                path: PathBuf::from("/tmp/stm/202610_sa2a7_alpha"),
            }],
        };
        let value = serde_json::to_value(failure.envelope()).unwrap();
        assert_eq!(value["error"]["code"], "sid-resolve-ambiguous");
        assert_eq!(value["error"]["candidates"][0]["id"], "202610_sa2a7_alpha");
        assert_eq!(
            value["error"]["candidates"][0]["path"],
            "/tmp/stm/202610_sa2a7_alpha"
        );
        let regular = Failure::ResolveInvalid("invalid".to_string());
        let value = serde_json::to_value(regular.envelope()).unwrap();
        assert!(value["error"].get("candidates").is_none());
    }
}
