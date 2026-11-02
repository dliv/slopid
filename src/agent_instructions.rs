use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AgentInstructions {
    pub format: &'static str,
    pub text: &'static str,
}

const FORMAT_MARKDOWN: &str = "markdown";
const TEXT: &str = "\
sid allocates and identifies local task folders with short refs such as sa2a7.

Use `sid init` to write a default `.sid` containing `root = \"stm\"`.
The nearest upward `.sid` accepts only top-level `root` and optional
`[ref].deny_prefixes`. Without a config, the current directory is the base and
the root defaults to `stm`. Root paths must be relative and may not contain
`..`. Generated refs use the built-in deny-prefix policy unless a configured
array replaces it; an empty array disables that policy.

Use `sid root` to read the absolute project base and logical identity root
without creating the root. Use `sid root --human` for the path alone.
Use `sid new \"task title\"` to allocate a folder and
`sid new --dry-run \"task title\"` to preview it without writing.
Use `sid list` to see direct identity directories, newest-touched first.
Filter with AND terms such as `sid list delivery triage` (slug substrings) or
`sid list sd` (ref prefix); use `--sort id` for ascending folder names.
Use `sid resolve sa2a7` for an exact recognized ref or
`sid resolve 202610_sa2a7_task-name` for one complete folder name. A short ref
with multiple real directories reports sorted candidates; the complete name
still resolves exactly.

List and resolve include only direct real directories. Files and symlinks are
excluded from navigation but still reserve refs during allocation and lint.
These commands inspect folder names and filesystem metadata only; they never
open `CURRENT_STATE.md` or other authored content. A missing root is empty.

Use `sid lint` to audit direct-child identity names. It reports an unrecognized
ref after a valid six-digit period, a ref reserved more than once, or a root or
entry it could not read. It never opens authored documents. The JSON report
contains `complete`, `healthy`, and `findings` with project-relative paths;
exit 0 means complete and healthy, exit 1 means complete with findings, and
exit 2 means the scan was incomplete.

Successful commands emit JSON by default. Pre-result errors emit one JSON
`error` object on stderr and leave stdout empty; CLI usage errors exit 2 and
other command errors exit 1. Help and version do not load project config.
Slopid owns folder identity and allocation; task documents, relationships,
notes, seeds, and lifecycle state belong to a consumer such as Slopdeck.
Do not invent refs by hand for durable task folders.
";

pub fn agent_instructions() -> AgentInstructions {
    AgentInstructions {
        format: FORMAT_MARKDOWN,
        text: TEXT,
    }
}
