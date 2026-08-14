use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AgentInstructions {
    pub format: &'static str,
    pub text: &'static str,
}

const FORMAT_MARKDOWN: &str = "markdown";
const TEXT: &str = "\
sid allocates local task folders with short refs such as sa2a7.

Use `sid init` to write the default project config when a repo does not have one.
Configuration has purpose-named task, seed, note, topic, queue, and ref tables.
Topic roots are explicit; seed and note roots default beneath the configured task
root. Generated refs default to the built-in prude deny-prefix list. A present
`[ref].deny_prefixes` array replaces it exactly; an empty array allows every
otherwise valid generated ref.
Use `sid root` to read the configured active task root as JSON without creating it.
Use `sid new \"task title\"` when starting a durable task folder.
Use `sid new --dry-run \"task title\"` to preview the folder path.
Use `sid new --into .slow \"task title\"` to allocate into a configured scan root.
Use `sid list` to see existing folders and reservations, newest-touched first.
Filter with terms: `sid list delivery triage` (slug words) or `sid list sd` (ref prefix).
Use `sid resolve se2vv` to resolve one exact, case-sensitive canonical frontmatter id.
Use `sid graph sdz85` to read its complete incoming and outgoing relationship component.
Use `sid lint` to audit the configured STM roots with stable finding codes.
Use `sid search mapper websocket` for literal AND search across authored text.
Use `sid context se2vv` for one task/review graph plus its pending inbox envelopes.
Use `sid captures` to inventory pending notes and seeds without returning note bodies.
Use `sid note \"raw thought\"` or pipe stdin to capture one identity-free note.
Inspect note `state`; suspected secrets return `quarantined` and are not echoed.
Use `sid seed \"parked idea\" --origin se2vv` to create a linkable seed.
Use `sid seed \"parked idea\" --dry-run` to validate without reading input or writing.
Use `sid new --from-seed sb3b8` to move the exact seed bytes into task `napkin.md`.
Use `sid relink` to preview proven Markdown destination repairs without writing.
After reviewing `complete`, `applied`, `changes`, and `findings`, use
`sid relink --write` to apply each still-verified source file independently.
Use `sid relink --move sa2a7 --into .archive` to preview only the destinations one
planned move would change: inbound links to that owner plus links authored inside
it. The preview adds `projection` and `plan_sha256`.
Then use `sid relink --move sa2a7 --into .archive --write --expected-plan-sha256 <plan_sha256>`
with the digest from a fresh preview. A projected write refuses
before touching any file when the plan changed or is incomplete, and a partial
result requires another preview rather than a rollback. When the owner already
sits under that root, the same command verifies its scoped links instead of
planning a move.
Projected `complete` means move-caused repair authority is safe; it is not a
certificate that every local destination resolves. A generic destination proven
absent under both current and projected readings yields
`relink-unresolved-local-destination` with `severity:warning`. The finding and
complete source bytes bind the digest, but no replacement is authorized and
completeness stays true. Exact-digest warning-only `--write` is valid zero-change
convergence. Every destination inside the move effect set,
including ref-less and multi-ref paths, is matched to one raw span independently
proven to decode to it, or it blocks completeness; each replacement is proven to reparse to the
intended destination before write authority. Generic projected changes use
`id:null`. Ambiguity, unknown target inspection, unsafe representation, and
actual move-caused drift remain errors. Settled verification judges
recognized-ref and generic destinations by unambiguous resolution to their intended target;
canonical spelling belongs to ordinary global relink. Recognized missing-target
and ambiguity errors remain blocking, while generic proven absence retains its
warning. Excluded inbox, note, tmp, VCS, and ignored paths remain
outside that proof boundary. Slopid reports only observed link and filesystem
state and does not model cleanup-resource lifecycle.
Keep authored Markdown writers quiescent from an approved projected `--write`
through the owner rename you perform afterwards. Slopid never detects or leases
authored writers: its per-file byte check skips a file whose bytes already
changed before the comparison, but a write landing after that check and before
atomic replacement is overwritten. Slopid only repairs Markdown destinations.
It never moves a folder, files a task, or performs any other lifecycle step.
The read/query commands are read-only and emit JSON by default. Inspect `complete`
and `findings` before treating partial context as authoritative. Parse canonical
node frontmatter instead of inferring identity or graph role from folder paths.
Apart from generated-prefix selection, ordinary `sid new` and `sid new --into`
destination semantics and `sid list` reader semantics are unchanged.
Cite the full sXXXX ref in notes and messages.
Do not invent refs by hand for durable task folders.
";

pub fn agent_instructions() -> AgentInstructions {
    AgentInstructions {
        format: FORMAT_MARKDOWN,
        text: TEXT,
    }
}
