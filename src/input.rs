use anyhow::{Context, Result, bail};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::process::Command;

#[derive(Debug, PartialEq, Eq)]
pub enum CollectedInput {
    Text(String),
    Cancelled,
}

pub fn collect_note(argument: Option<String>) -> Result<CollectedInput> {
    if let Some(text) = argument {
        return Ok(nonempty(text));
    }
    if !io::stdin().is_terminal() {
        return read_stdin().map(nonempty);
    }
    edit_buffer().map(nonempty)
}

pub fn collect_seed_body(edit: bool) -> Result<String> {
    if edit {
        return edit_buffer();
    }
    if io::stdin().is_terminal() {
        Ok(String::new())
    } else {
        read_stdin()
    }
}

fn nonempty(text: String) -> CollectedInput {
    if text.is_empty() {
        CollectedInput::Cancelled
    } else {
        CollectedInput::Text(text)
    }
}

fn read_stdin() -> Result<String> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .context("read capture text from stdin")?;
    Ok(text)
}

fn edit_buffer() -> Result<String> {
    let mut temporary = tempfile::NamedTempFile::new().context("create editor recovery file")?;
    temporary.flush().context("flush editor recovery file")?;
    let (_file, path) = temporary
        .keep()
        .map_err(|err| err.error)
        .context("preserve editor recovery file")?;
    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "vi".to_string());
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("vi");
    let status = Command::new(program)
        .args(parts)
        .arg(&path)
        .status()
        .with_context(|| format!("launch editor; recovery file kept at {}", path.display()))?;
    if !status.success() {
        bail!(
            "editor exited with {status}; recovery file kept at {}",
            path.display()
        );
    }
    let text = fs::read_to_string(&path).with_context(|| {
        format!(
            "read editor buffer; recovery file kept at {}",
            path.display()
        )
    })?;
    fs::remove_file(&path)
        .with_context(|| format!("remove successful editor buffer {}", path.display()))?;
    Ok(text)
}
