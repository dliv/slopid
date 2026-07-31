use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

fn sid() -> assert_cmd::Command {
    cargo_bin_cmd!("sid")
}

fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

fn write_canonical(path: &Path, frontmatter: &str, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, format!("---\n{frontmatter}---\n{body}")).unwrap();
}

fn keys(value: &Value) -> Vec<&str> {
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

fn typed_project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join(".sid"),
        "[task]\nroot = \"work\"\nscan_roots = [\"history\"]\n[seed]\nroot = \"parking\"\n[note]\nroot = \"scratch/notes\"\n[topic]\nroots = [\"knowledge\"]\n[queue]\nstale_after_days = 11\n",
    )
    .unwrap();
    write_canonical(
        &tmp.path().join("history/202401_816d_old/CURRENT_STATE.md"),
        "type: \"review\"\nid: \"816d\"\ntitle: \"Old review\"\ntimestamp: \"2024-01-01\"\n",
        "",
    );
    write_canonical(
        &tmp.path().join("work/202402_sa2a7_task/CURRENT_STATE.md"),
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"Task\"\ntimestamp: \"2024-02-01\"\norigin: [\"816d\"]\n",
        "## Related Work\n- 816d\n",
    );
    write_canonical(
        &tmp.path().join("parking/202403_sb3b8_parked.md"),
        "type: \"seed\"\nid: \"sb3b8\"\ntitle: \"Parked\"\ntimestamp: \"2024-03-01\"\norigin: [\"sa2a7\"]\n",
        "## Related Work\n- sa2a7\n",
    );
    write_canonical(
        &tmp.path().join("knowledge/nested/guide.md"),
        "type: \"topic\"\nid: \"topic/guide\"\ntitle: \"Guide\"\ntimestamp: \"2024-04-01\"\nrelated: [\"sb3b8\"]\n",
        "## Related Work\n- sb3b8\n",
    );
    tmp
}

/// Projected moves need a task archive lane, a review archive lane, and a
/// second task so an outbound link authored inside the moving owner also moves.
fn move_project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join(".sid"),
        "[task]\nroot = \"work\"\nscan_roots = [\"work/.archive\", \"prs\", \"prs/.archive-prs\"]\n[seed]\nroot = \"parking\"\n[note]\nroot = \"scratch/notes\"\n[topic]\nroots = [\"knowledge\"]\n",
    )
    .unwrap();
    write_canonical(
        &tmp.path().join("work/202402_sa2a7_task/CURRENT_STATE.md"),
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"Task\"\ntimestamp: \"2024-02-01\"\n",
        "",
    );
    write_canonical(
        &tmp.path().join("work/202404_sd5d2_other/CURRENT_STATE.md"),
        "type: \"task\"\nid: \"sd5d2\"\ntitle: \"Other\"\ntimestamp: \"2024-04-01\"\n",
        "",
    );
    write_canonical(
        &tmp.path().join("prs/202401_816d_review/CURRENT_STATE.md"),
        "type: \"review\"\nid: \"816d\"\ntitle: \"Review\"\ntimestamp: \"2024-01-01\"\n",
        "",
    );
    write_markdown(
        &tmp.path().join("prs/202401_816d_review/notes.md"),
        "# Notes\n",
    );
    write_canonical(
        &tmp.path().join("parking/202403_sb3b8_parked.md"),
        "type: \"seed\"\nid: \"sb3b8\"\ntitle: \"Parked\"\ntimestamp: \"2024-03-01\"\n",
        "",
    );
    write_canonical(
        &tmp.path().join("knowledge/nested/guide.md"),
        "type: \"topic\"\nid: \"topic/guide\"\ntitle: \"Guide\"\ntimestamp: \"2024-04-01\"\n",
        "",
    );
    tmp
}

fn write_markdown(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
}

fn text_of(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn mtime_of(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

fn projected_preview(project: &Path, id: &str, into: &str) -> Value {
    let output = sid()
        .args(["relink", "--move", id, "--into", into])
        .current_dir(project)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    json(&output)
}

fn projected_write(project: &Path, id: &str, into: &str, digest: &str) -> Value {
    let output = sid()
        .args([
            "relink",
            "--move",
            id,
            "--into",
            into,
            "--write",
            "--expected-plan-sha256",
            digest,
        ])
        .current_dir(project)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    json(&output)
}

fn plan_digest(value: &Value) -> String {
    let digest = value["plan_sha256"].as_str().unwrap();
    assert_eq!(
        digest.len(),
        64,
        "plan digest must be sha-256 hex: {digest}"
    );
    assert!(
        digest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')),
        "plan digest must be lowercase hex: {digest}"
    );
    digest.to_string()
}

fn finding_codes(value: &Value) -> Vec<&str> {
    value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect()
}

fn change_pairs(value: &Value) -> Vec<(String, String)> {
    value["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|change| {
            (
                change["from"].as_str().unwrap().to_string(),
                change["to"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn configure_relink_extensions(project: &Path, extensions: &[&str]) {
    let path = project.join(".sid");
    let mut config = fs::read_to_string(&path).unwrap();
    let extensions = extensions
        .iter()
        .map(|extension| format!("\"{extension}\""))
        .collect::<Vec<_>>()
        .join(", ");
    config.push_str(&format!(
        "\n[relink]\ndestination_extensions = [{extensions}]\n"
    ));
    fs::write(path, config).unwrap();
}

fn global_relink(project: &Path, write: bool) -> Value {
    let mut command = sid();
    command.arg("relink");
    if write {
        command.arg("--write");
    }
    let output = command
        .current_dir(project)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    json(&output)
}

#[test]
fn resolve_and_graph_compose_task_review_seed_and_topic_sources() {
    let tmp = typed_project();
    for id in ["816d", "sa2a7", "sb3b8", "topic/guide"] {
        let output = sid()
            .args(["resolve", id])
            .current_dir(tmp.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert_eq!(json(&output)["node"]["frontmatter"]["id"], id);
    }

    let output = sid()
        .args(["graph", "topic/guide"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["complete"], true);
    assert_eq!(
        value["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["frontmatter"]["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["816d", "sa2a7", "sb3b8", "topic/guide"]
    );
}

#[test]
fn omitted_typed_tables_default_beneath_custom_task_root() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join(".sid"), "[task]\nroot = \"memory\"\n").unwrap();
    write_canonical(
        &tmp.path().join("memory/.seeds/202403_sb3b8_parked.md"),
        "type: \"seed\"\nid: \"sb3b8\"\ntitle: \"Parked\"\ntimestamp: \"2024-03-01\"\n",
        "",
    );
    let output = sid()
        .args(["resolve", "sb3b8"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(json(&output)["node"]["frontmatter"]["id"], "sb3b8");
}

#[test]
fn malformed_seed_and_topic_are_omitted_without_hiding_healthy_nodes() {
    let tmp = typed_project();
    write_canonical(
        &tmp.path().join("parking/202405_sc4c9_wrong.md"),
        "type: \"seed\"\nid: \"different\"\ntitle: \"Wrong\"\ntimestamp: \"2024-05-01\"\n",
        "",
    );
    write_canonical(
        &tmp.path().join("knowledge/bad.md"),
        "type: \"task\"\nid: \"sc4c9\"\ntitle: \"Wrong kind\"\ntimestamp: \"2024-05-02\"\n",
        "",
    );
    let lint = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let value = json(&lint);
    let codes = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"id-folder-mismatch"));
    assert!(codes.contains(&"unsupported-type"));

    let healthy = sid()
        .args(["resolve", "sa2a7"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(json(&healthy)["node"]["frontmatter"]["id"], "sa2a7");
}

#[test]
fn search_groups_ranked_owners_and_excludes_verbatim_queues() {
    let tmp = typed_project();
    fs::write(
        tmp.path().join("work/202402_sa2a7_task/design.txt"),
        "Alpha protocol appears here.\nSecond alpha protocol line.\n",
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("work/202402_sa2a7_task/inbox")).unwrap();
    fs::write(
        tmp.path().join("work/202402_sa2a7_task/inbox/private.md"),
        "alpha protocol must stay invisible",
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("scratch/notes/quarantine")).unwrap();
    fs::write(
        tmp.path().join("scratch/notes/pending.md"),
        "alpha protocol from a raw note",
    )
    .unwrap();
    fs::write(
        tmp.path().join("scratch/notes/quarantine/hidden.md"),
        "alpha protocol secret",
    )
    .unwrap();
    fs::write(
        tmp.path().join("scratch/notes/log.md"),
        "alpha protocol filing ledger",
    )
    .unwrap();
    fs::write(tmp.path().join("work/.gitignore"), "ignored.txt\n").unwrap();
    fs::write(
        tmp.path().join("work/202402_sa2a7_task/ignored.txt"),
        "alpha protocol ignored",
    )
    .unwrap();

    let output = sid()
        .args(["search", "alpha", "protocol"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(keys(&value), ["complete", "findings", "results", "total"]);
    assert_eq!(value["complete"], true);
    assert_eq!(value["total"], 2);
    let results = value["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["owner_kind"], "canonical");
    assert_eq!(
        keys(&results[0]),
        [
            "excerpts",
            "match_count",
            "node",
            "owner_kind",
            "path",
            "rank"
        ]
    );
    assert_eq!(results[0]["node"]["frontmatter"]["id"], "sa2a7");
    assert_eq!(results[0]["rank"], "body");
    assert_eq!(results[1]["owner_kind"], "note");
    assert_eq!(
        keys(&results[0]["excerpts"][0]),
        ["line", "path", "text", "truncated"]
    );
    assert!(
        results
            .iter()
            .flat_map(|result| result["excerpts"].as_array().unwrap())
            .all(
                |excerpt| !excerpt["path"].as_str().unwrap().contains("inbox")
                    && !excerpt["path"].as_str().unwrap().contains("quarantine")
                    && !excerpt["path"].as_str().unwrap().contains("ignored.txt")
            )
    );
}

#[test]
fn search_caps_excerpts_and_unicode_scalars_at_exact_boundaries() {
    let tmp = typed_project();
    let long = format!("{}needleunique", "é".repeat(235));
    fs::write(
        tmp.path().join("work/202402_sa2a7_task/long.txt"),
        format!("{long}\nneedleunique two\nneedleunique three\nneedleunique four\n"),
    )
    .unwrap();
    let output = sid()
        .args(["search", "needleunique"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    let result = value["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|result| result["node"]["frontmatter"]["id"] == "sa2a7")
        .unwrap();
    assert_eq!(result["excerpts"].as_array().unwrap().len(), 3);
    assert_eq!(result["excerpts"][0]["truncated"], true);
    let text = result["excerpts"][0]["text"].as_str().unwrap();
    assert!(text.chars().count() <= 240, "cap holds: {text}");
    // The term sits past the cap, where a prefix cut would have dropped it.
    assert!(text.contains("needleunique"), "match must survive: {text}");
    assert!(text.starts_with('…'), "left edge is elided: {text}");
}

#[test]
fn search_uses_nearest_nested_repository_ignore_boundary() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".git")).unwrap();
    fs::write(tmp.path().join(".gitignore"), "nested/\n").unwrap();
    fs::create_dir_all(tmp.path().join("nested/.git")).unwrap();
    fs::write(
        tmp.path().join(".sid"),
        "[task]\nroot = \"nested/tasks\"\nscan_roots = []\n",
    )
    .unwrap();
    write_canonical(
        &tmp.path()
            .join("nested/tasks/202401_sa2a7_task/CURRENT_STATE.md"),
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"Nested\"\ntimestamp: \"2024-01-01\"\n",
        "boundaryneedle\n",
    );
    let output = sid()
        .args(["search", "boundaryneedle"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(json(&output)["total"], 1);
}

#[test]
fn search_without_any_existing_source_fails_with_empty_stdout() {
    let tmp = tempfile::tempdir().unwrap();
    let output = sid()
        .args(["search", "anything"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
}

#[test]
fn search_id_rank_limit_and_argument_boundaries_are_exact() {
    let tmp = typed_project();
    let output = sid()
        .args(["search", "sa2a7", "--limit", "1"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert!(value["total"].as_u64().unwrap() >= 1);
    assert_eq!(value["results"].as_array().unwrap().len(), 1);
    assert_eq!(value["results"][0]["rank"], "id");
    for args in [vec!["search"], vec!["search", "x", "--limit", "0"]] {
        let output = sid()
            .args(args)
            .current_dir(tmp.path())
            .assert()
            .failure()
            .get_output()
            .clone();
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn captures_returns_metadata_only_and_newest_first() {
    let tmp = typed_project();
    fs::create_dir_all(tmp.path().join("scratch/notes")).unwrap();
    let old = tmp.path().join("scratch/notes/old.md");
    let new = tmp.path().join("scratch/notes/new.md");
    fs::write(&old, "never echo old body").unwrap();
    fs::write(&new, "never echo new body").unwrap();
    fs::write(
        tmp.path().join("scratch/notes/log.md"),
        "never echo filing ledger",
    )
    .unwrap();
    let old_file = fs::OpenOptions::new().write(true).open(&old).unwrap();
    old_file
        .set_times(
            fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(100)),
        )
        .unwrap();
    let new_file = fs::OpenOptions::new().write(true).open(&new).unwrap();
    new_file
        .set_times(
            fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(200)),
        )
        .unwrap();

    let output = sid()
        .arg("captures")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(keys(&value), ["complete", "findings", "notes", "seeds"]);
    assert_eq!(value["complete"], true);
    assert_eq!(value["notes"].as_array().unwrap().len(), 2);
    assert!(
        value["notes"][0]["path"]
            .as_str()
            .unwrap()
            .ends_with("new.md")
    );
    assert_eq!(keys(&value["notes"][0]), ["bytes", "modified", "path"]);
    assert_eq!(value["seeds"][0]["frontmatter"]["id"], "sb3b8");
    assert!(!String::from_utf8(output).unwrap().contains("never echo"));
}

#[test]
fn context_embeds_graph_and_pending_inbox_envelopes_without_bodies() {
    let tmp = typed_project();
    let inbox = tmp.path().join("work/202402_sa2a7_task/inbox");
    fs::create_dir_all(inbox.join("done")).unwrap();
    write_canonical(
        &inbox.join("later.md"),
        "from: \"sb3b8\"\ndate: \"2026-07-13\"\nsubject: \"Later\"\nextra: {\"kept\": true}\n",
        "secret body later",
    );
    write_canonical(
        &inbox.join("earlier.md"),
        "from: \"816d\"\ndate: \"2026-07-12\"\nsubject: \"Earlier\"\n",
        "secret body earlier",
    );
    write_canonical(
        &inbox.join("done/filed.md"),
        "from: \"816d\"\ndate: \"2020-01-01\"\nsubject: \"Done\"\n",
        "secret filed body",
    );

    let output = sid()
        .args(["context", "sa2a7"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(keys(&value), ["complete", "graph", "inbox", "node"]);
    assert_eq!(value["node"]["frontmatter"]["id"], "sa2a7");
    assert_eq!(value["graph"]["anchor"], "sa2a7");
    assert_eq!(
        value["inbox"]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| message["frontmatter"]["subject"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Earlier", "Later"]
    );
    let text = String::from_utf8(output).unwrap();
    assert!(!text.contains("secret body"));
    assert!(!text.contains("filed"));

    for id in ["sb3b8", "topic/guide"] {
        let output = sid()
            .args(["context", id])
            .current_dir(tmp.path())
            .assert()
            .failure()
            .get_output()
            .clone();
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn invalid_inbox_is_partial_context_and_a_lint_data_error() {
    let tmp = typed_project();
    let inbox = tmp.path().join("work/202402_sa2a7_task/inbox");
    fs::create_dir_all(&inbox).unwrap();
    write_canonical(
        &inbox.join("bad.md"),
        "from: \"816d\"\ndate: \"not-a-date\"\nsubject: \"Bad\"\n",
        "body",
    );
    let context = sid()
        .args(["context", "sa2a7"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&context);
    assert_eq!(value["complete"], false);
    assert_eq!(value["inbox"]["complete"], false);
    assert!(
        value["inbox"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "invalid-inbox-envelope")
    );

    let lint = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    assert!(
        json(&lint)["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "invalid-inbox-envelope")
    );
}

#[test]
fn stale_queue_findings_are_warning_only_for_lint() {
    let tmp = typed_project();
    let inbox = tmp.path().join("work/202402_sa2a7_task/inbox");
    fs::create_dir_all(&inbox).unwrap();
    write_canonical(
        &inbox.join("old.md"),
        "from: \"816d\"\ndate: \"2020-01-01\"\nsubject: \"Old\"\n",
        "body",
    );
    let output = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    let stale = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["code"] == "stale-inbox-message")
        .unwrap();
    assert_eq!(stale["severity"], "warning");
}

#[test]
fn help_and_agent_instructions_describe_read_query_commands() {
    let root = sid()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let root = String::from_utf8(root).unwrap();
    for phrase in [
        "Read/query:",
        "Controlled mutation:",
        "relink previews",
        "inspect complete, applied, changes, and findings",
    ] {
        assert!(root.contains(phrase), "root help omitted {phrase}");
    }
    for command in ["search", "context", "captures", "note", "seed", "relink"] {
        assert!(root.contains(command));
        let help = sid()
            .args([command, "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(String::from_utf8(help).unwrap().contains("JSON"));
    }

    let relink_help = sid()
        .args(["relink", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let relink_help = String::from_utf8(relink_help).unwrap();
    for phrase in [
        "--move <ID>",
        "--into <ROOT>",
        "--expected-plan-sha256 <SHA256>",
        "default is preview",
        "fresh preview",
    ] {
        assert!(relink_help.contains(phrase), "relink help omitted {phrase}");
    }
    let output = sid()
        .arg("agent-instructions")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = json(&output)["text"].as_str().unwrap().to_string();
    for phrase in [
        "sid search",
        "sid context",
        "sid captures",
        "complete",
        "note bodies",
        "sid note",
        "sid seed",
        "sid new --from-seed",
        "quarantined",
        "sid relink",
        "sid relink --write",
        "sid relink --move sa2a7 --into .archive",
        "--expected-plan-sha256",
        "refuses",
        "every local destination",
        "including ref-less and multi-ref paths",
        "outside that proof boundary",
        "id:null",
        "never moves a folder",
        "applied",
        "[ref].deny_prefixes",
        "Apart from generated-prefix selection",
        "`sid list` reader semantics are unchanged",
    ] {
        assert!(text.contains(phrase), "missing {phrase}");
    }
    assert!(
        text.contains(
            "sid relink --move sa2a7 --into .archive --write --expected-plan-sha256 <plan_sha256>"
        ),
        "the digest-bound write command must render as one unbroken command"
    );
    assert!(!text.contains("task semantics are unchanged"));
}

#[test]
fn note_argument_stdin_cancellation_and_quarantine_do_not_echo_content() {
    let tmp = typed_project();
    let pending = sid()
        .args(["note", "ordinary capture text"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let pending_json = json(&pending);
    assert_eq!(keys(&pending_json), ["path", "state"]);
    assert_eq!(pending_json["state"], "pending");
    let pending_path = Path::new(pending_json["path"].as_str().unwrap());
    assert_eq!(
        fs::read_to_string(pending_path).unwrap(),
        "ordinary capture text"
    );
    assert!(
        !String::from_utf8(pending)
            .unwrap()
            .contains("ordinary capture text")
    );

    let quarantined = sid()
        .arg("note")
        .write_stdin("password: super-secret-value")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let quarantined_json = json(&quarantined);
    assert_eq!(quarantined_json["state"], "quarantined");
    assert!(
        quarantined_json["path"]
            .as_str()
            .unwrap()
            .contains("/quarantine/")
    );
    assert!(
        !String::from_utf8(quarantined)
            .unwrap()
            .contains("super-secret")
    );

    let cancelled = sid()
        .arg("note")
        .write_stdin("")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        json(&cancelled),
        serde_json::json!({"state": "cancelled", "path": null})
    );
}

#[test]
fn seed_writes_minimum_valid_frontmatter_body_and_origin_lines() {
    let tmp = typed_project();
    let output = sid()
        .args(["seed", "A parked idea", "--origin", "sa2a7"])
        .write_stdin("Body bytes stay here.\n")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(
        keys(&value),
        [
            "dry_run", "id", "path", "period", "sid_ref", "slug", "title"
        ]
    );
    assert_eq!(value["dry_run"], false);
    let path = Path::new(value["path"].as_str().unwrap());
    assert_eq!(
        path.extension().and_then(|value| value.to_str()),
        Some("md")
    );
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains("type: \"seed\""));
    assert!(text.contains(&format!("id: \"{}\"", value["sid_ref"].as_str().unwrap())));
    assert!(text.contains("title: \"A parked idea\""));
    assert!(text.contains("origin: [\"sa2a7\"]"));
    assert!(text.contains("- `sa2a7` — Origin: This seed was captured from `sa2a7`."));
    assert!(text.ends_with("Body bytes stay here.\n"));

    let minimum = sid()
        .args(["seed", "Minimum concept"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let minimum = json(&minimum);
    let minimum_text = fs::read_to_string(minimum["path"].as_str().unwrap()).unwrap();
    assert!(minimum_text.ends_with("---\n"));
}

#[test]
fn seed_dry_run_is_input_inert_and_origin_validation_is_exact() {
    let tmp = typed_project();
    let output = sid()
        .args([
            "seed",
            "Preview",
            "--origin",
            "sa2a7",
            "--edit",
            "--dry-run",
        ])
        .env("VISUAL", "false")
        .write_stdin("must not be consumed or written")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["dry_run"], true);
    assert!(!Path::new(value["path"].as_str().unwrap()).exists());

    for args in [
        vec!["seed", "Bad origin", "--origin", "missing"],
        vec![
            "seed",
            "Duplicate",
            "--origin",
            "sa2a7",
            "--origin",
            "sa2a7",
        ],
    ] {
        let output = sid()
            .args(args)
            .current_dir(tmp.path())
            .assert()
            .failure()
            .get_output()
            .clone();
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn seed_editor_failure_keeps_recovery_file_and_creates_no_seed() {
    let tmp = typed_project();
    let recovery = tmp.path().join("editor-recovery");
    fs::create_dir_all(&recovery).unwrap();
    let output = sid()
        .args(["seed", "Editor failure", "--edit"])
        .env("VISUAL", "false")
        .env("TMPDIR", &recovery)
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("recovery file kept")
    );
    assert_eq!(fs::read_dir(&recovery).unwrap().count(), 1);
}

#[test]
fn graduation_moves_exact_seed_bytes_to_napkin_and_preserves_identity() {
    let tmp = typed_project();
    let seed = sid()
        .args(["seed", "Graduate me"])
        .write_stdin("exact seed body\n")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let seed = json(&seed);
    let seed_path = Path::new(seed["path"].as_str().unwrap()).to_path_buf();
    let bytes = fs::read(&seed_path).unwrap();
    let sid_ref = seed["sid_ref"].as_str().unwrap();

    let preview = sid()
        .args(["new", "--from-seed", sid_ref, "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let preview = json(&preview);
    assert_eq!(preview["sid_ref"], sid_ref);
    assert_eq!(preview["dry_run"], true);
    assert!(seed_path.exists());
    assert!(!Path::new(preview["path"].as_str().unwrap()).exists());

    let output = sid()
        .args(["new", "--from-seed", sid_ref])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["sid_ref"], sid_ref);
    assert_eq!(value["slug"], "graduate-me");
    let task = Path::new(value["path"].as_str().unwrap());
    assert_eq!(fs::read(task.join("napkin.md")).unwrap(), bytes);
    assert!(!seed_path.exists());
    assert!(!task.join("CURRENT_STATE.md").exists());
}

#[test]
fn new_requires_exactly_title_or_from_seed_and_rejects_destination_collision() {
    let tmp = typed_project();
    for args in [vec!["new"], vec!["new", "title", "--from-seed", "sb3b8"]] {
        let output = sid()
            .args(args)
            .current_dir(tmp.path())
            .assert()
            .failure()
            .get_output()
            .clone();
        assert!(output.stdout.is_empty());
    }

    fs::create_dir_all(tmp.path().join("work/202403_sb3b8_parked")).unwrap();
    let original = fs::read(tmp.path().join("parking/202403_sb3b8_parked.md")).unwrap();
    let output = sid()
        .args(["new", "--from-seed", "sb3b8"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
    assert_eq!(
        fs::read(tmp.path().join("parking/202403_sb3b8_parked.md")).unwrap(),
        original
    );
}

#[test]
fn relink_preview_and_write_repair_task_seed_image_and_reference_destinations() {
    let tmp = typed_project();
    let source = tmp.path().join("knowledge/links.md");
    fs::write(
        &source,
        "[task](../old/202402_sa2a7_old/CURRENT_STATE.md#part \"title\")\n\
![seed](<../old/202403_sb3b8_old.md>)\n\
[reference][task-ref]\n\n\
[task-ref]: ../old/202401_816d_old/CURRENT_STATE.md\n\n\
```md\n[code](../old/202402_sa2a7_old/CURRENT_STATE.md)\n```\n",
    )
    .unwrap();
    let before = fs::read(&source).unwrap();
    let before_mtime = fs::metadata(&source).unwrap().modified().unwrap();
    let preview = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let preview = json(&preview);
    assert_eq!(
        keys(&preview),
        ["applied", "changes", "complete", "findings"]
    );
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["complete"], true);
    assert_eq!(preview["changes"].as_array().unwrap().len(), 3);
    assert_eq!(
        keys(&preview["changes"][0]),
        ["column", "from", "id", "line", "path", "to"]
    );
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(
        fs::metadata(&source).unwrap().modified().unwrap(),
        before_mtime
    );

    let written = sid()
        .args(["relink", "--write"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let written = json(&written);
    assert_eq!(
        keys(&written),
        ["applied", "changes", "complete", "findings"],
        "global relink --write must not gain projected-only keys"
    );
    assert_eq!(written["applied"], true);
    assert_eq!(written["complete"], true);
    assert_eq!(written["changes"].as_array().unwrap().len(), 3);
    let text = fs::read_to_string(&source).unwrap();
    assert!(text.contains("../work/202402_sa2a7_task/CURRENT_STATE.md#part"));
    assert!(text.contains("../parking/202403_sb3b8_parked.md"));
    assert!(text.contains("../history/202401_816d_old/CURRENT_STATE.md"));
    assert!(text.contains("[code](../old/202402_sa2a7_old/CURRENT_STATE.md)"));
}

#[test]
fn relink_write_preserves_required_commonmark_destination_escapes() {
    let tmp = typed_project();
    let target = tmp.path().join("work/202402_sa2a7_task/close).md");
    write_markdown(&target, "# Close\n");
    let canonical = tmp.path().join("knowledge/canonical-escape.md");
    let stale = tmp.path().join("knowledge/stale-escape.md");
    write_markdown(
        &canonical,
        "[close](../work/202402_sa2a7_task/close\\).md)\n",
    );
    write_markdown(&stale, "[close](../old/202402_sa2a7_old/close\\).md)\n");

    let preview = global_relink(tmp.path(), false);
    assert_eq!(preview["complete"], true);
    assert_eq!(
        change_pairs(&preview),
        [(
            "../old/202402_sa2a7_old/close\\).md".to_string(),
            "../work/202402_sa2a7_task/close\\).md".to_string(),
        )],
        "canonical escaped text must compare semantically and repaired text must retain the load-bearing escape"
    );

    let written = global_relink(tmp.path(), true);
    assert_eq!(written["complete"], true);
    assert_eq!(written["changes"][0]["id"], "sa2a7");
    assert_eq!(
        text_of(&canonical),
        "[close](../work/202402_sa2a7_task/close\\).md)\n"
    );
    assert_eq!(
        text_of(&stale),
        "[close](../work/202402_sa2a7_task/close\\).md)\n"
    );
    let reparsed = global_relink(tmp.path(), false);
    assert_eq!(reparsed["complete"], true);
    assert!(reparsed["changes"].as_array().unwrap().is_empty());
    assert!(target.exists());
}

#[test]
fn relink_projected_commonmark_representation_is_semantic_and_write_safe() {
    let tmp = move_project();
    for name in ["a(b).md", "close).md", "a(b.md"] {
        write_markdown(
            &tmp.path().join("work/202402_sa2a7_task").join(name),
            "# Target\n",
        );
    }
    write_markdown(
        &tmp.path().join("work/202404_sd5d2_other/close).md"),
        "# Other\n",
    );
    let inbound = tmp.path().join("knowledge/commonmark.md");
    write_markdown(
        &inbound,
        "[balanced](../work/202402_sa2a7_task/a\\(b\\).md)\n\
[close](../work/202402_sa2a7_task/close\\).md)\n\
[open](../work/202402_sa2a7_task/a\\(b.md)\n\
[angle](<../work/202402_sa2a7_task/close).md>)\n\
[entity](../work/202402_sa2a7_task/a&#40;b&#41;.md)\n\
[reference][target]\n\n\
[target]: <../work/202402_sa2a7_task/a(b.md>\n",
    );
    let outbound = tmp.path().join("work/202402_sa2a7_task/outbound.md");
    write_markdown(&outbound, "[other](../202404_sd5d2_other/close\\).md)\n");

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(preview["complete"], true, "{preview:#}");
    assert!(finding_codes(&preview).is_empty(), "{preview:#}");
    let pairs = change_pairs(&preview);
    assert!(
        pairs
            .iter()
            .any(|(_, to)| { to == "../work/.archive/202402_sa2a7_task/a(b).md" }),
        "{pairs:#?}"
    );
    assert!(
        pairs
            .iter()
            .any(|(_, to)| { to == "../work/.archive/202402_sa2a7_task/close\\).md" }),
        "{pairs:#?}"
    );
    assert!(
        pairs
            .iter()
            .any(|(_, to)| { to == "../work/.archive/202402_sa2a7_task/a\\(b.md" }),
        "{pairs:#?}"
    );
    assert!(
        pairs
            .iter()
            .any(|(_, to)| { to == "../../202404_sd5d2_other/close\\).md" }),
        "{pairs:#?}"
    );

    let applied = projected_write(tmp.path(), "sa2a7", ".archive", &plan_digest(&preview));
    assert_eq!(applied["complete"], true, "{applied:#}");
    fs::create_dir_all(tmp.path().join("work/.archive")).unwrap();
    fs::rename(
        tmp.path().join("work/202402_sa2a7_task"),
        tmp.path().join("work/.archive/202402_sa2a7_task"),
    )
    .unwrap();

    let reparsed = global_relink(tmp.path(), false);
    assert_eq!(reparsed["complete"], true, "{reparsed:#}");
    assert!(
        reparsed["changes"].as_array().unwrap().is_empty(),
        "{reparsed:#}"
    );
}

#[test]
fn relink_colon_line_is_literal_by_default_and_empty_config_is_equivalent() {
    let tmp = typed_project();
    let source = tmp.path().join("knowledge/colon-line-default.md");
    let authored = "[task](../old/202402_sa2a7_old/CURRENT_STATE.md:33)\n";
    fs::write(&source, authored).unwrap();

    let omitted = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let omitted_json = json(&omitted);
    assert_eq!(omitted_json["complete"], true);
    assert_eq!(omitted_json["applied"], false);
    assert!(omitted_json["changes"].as_array().unwrap().is_empty());
    let finding = omitted_json["findings"]
        .as_array()
        .unwrap()
        .first()
        .unwrap();
    assert_eq!(finding["code"], "relink-missing-internal-target");
    assert_eq!(finding["id"], "sa2a7");
    assert!(
        finding["message"]
            .as_str()
            .unwrap()
            .ends_with("work/202402_sa2a7_task/CURRENT_STATE.md:33")
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), authored);

    configure_relink_extensions(tmp.path(), &[]);
    let explicit_empty = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(explicit_empty, omitted);
    assert_eq!(fs::read_to_string(source).unwrap(), authored);
}

#[test]
fn relink_colon_line_preview_and_write_preserve_the_exact_locator() {
    let tmp = typed_project();
    configure_relink_extensions(tmp.path(), &["colon-line"]);
    let source = tmp.path().join("knowledge/colon-line.md");
    let authored = "[task](../old/202402_sa2a7_old/CURRENT_STATE.md:33)\n";
    fs::write(&source, authored).unwrap();

    let preview = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let preview = json(&preview);
    assert_eq!(preview["complete"], true);
    assert_eq!(preview["applied"], false);
    assert!(preview["findings"].as_array().unwrap().is_empty());
    assert_eq!(
        preview["changes"],
        serde_json::json!([{
            "path": source.canonicalize().unwrap(),
            "line": 1,
            "column": 8,
            "id": "sa2a7",
            "from": "../old/202402_sa2a7_old/CURRENT_STATE.md:33",
            "to": "../work/202402_sa2a7_task/CURRENT_STATE.md:33"
        }])
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), authored);

    let written = sid()
        .args(["relink", "--write"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let written = json(&written);
    assert_eq!(written["complete"], true);
    assert_eq!(written["applied"], true);
    assert_eq!(written["changes"], preview["changes"]);
    assert!(written["findings"].as_array().unwrap().is_empty());
    assert_eq!(
        fs::read_to_string(source).unwrap(),
        "[task](../work/202402_sa2a7_task/CURRENT_STATE.md:33)\n"
    );
}

#[test]
fn relink_colon_line_preserves_fragments_and_skips_external_ports() {
    let tmp = typed_project();
    configure_relink_extensions(tmp.path(), &["colon-line"]);
    let source = tmp.path().join("knowledge/colon-line-fragment.md");
    fs::write(
        &source,
        "[fragment](../old/202402_sa2a7_old/CURRENT_STATE.md:33#part)\n\
[first-line](../old/202402_sa2a7_old/CURRENT_STATE.md:1)\n\
[zero](../old/202402_sa2a7_old/CURRENT_STATE.md:0)\n\
[leading-zero](../old/202402_sa2a7_old/CURRENT_STATE.md:01)\n\
[missing](../old/202402_sa2a7_old/missing.md:33)\n\
[external](https://example.test:443/202402_sa2a7_old/CURRENT_STATE.md:33#part)\n",
    )
    .unwrap();

    let output = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["complete"], true);
    assert_eq!(
        value["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|change| (
                change["from"].as_str().unwrap(),
                change["to"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            (
                "../old/202402_sa2a7_old/CURRENT_STATE.md:33#part",
                "../work/202402_sa2a7_task/CURRENT_STATE.md:33#part"
            ),
            (
                "../old/202402_sa2a7_old/CURRENT_STATE.md:1",
                "../work/202402_sa2a7_task/CURRENT_STATE.md:1"
            )
        ]
    );
    assert_eq!(
        value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| finding["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "relink-missing-internal-target",
            "relink-missing-internal-target",
            "relink-missing-internal-target"
        ]
    );
    let messages = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["message"].as_str().unwrap())
        .collect::<Vec<_>>();
    for literal in ["CURRENT_STATE.md:0", "CURRENT_STATE.md:01", "missing.md:33"] {
        assert!(
            messages.iter().any(|message| message.ends_with(literal)),
            "missing finding for {literal}"
        );
    }
}

#[test]
fn relink_colon_line_skips_digit_bearing_uri_schemes_before_preview_and_write() {
    let tmp = typed_project();
    configure_relink_extensions(tmp.path(), &["colon-line"]);
    let source = tmp.path().join("knowledge/colon-line-external-scheme.md");
    let authored = "[external](s3://example.test:443/202402_sa2a7_old/CURRENT_STATE.md:33#part)\n";
    fs::write(&source, authored).unwrap();

    let preview = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let preview = json(&preview);
    assert_eq!(preview["complete"], true);
    assert_eq!(preview["applied"], false);
    assert!(preview["changes"].as_array().unwrap().is_empty());
    assert!(preview["findings"].as_array().unwrap().is_empty());
    assert_eq!(fs::read_to_string(&source).unwrap(), authored);

    let written = sid()
        .args(["relink", "--write"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let written = json(&written);
    assert_eq!(written["complete"], true);
    assert_eq!(written["applied"], true);
    assert!(written["changes"].as_array().unwrap().is_empty());
    assert!(written["findings"].as_array().unwrap().is_empty());
    assert_eq!(fs::read_to_string(source).unwrap(), authored);
}

#[test]
fn relink_colon_line_prefers_an_existing_literal_colon_filename() {
    let tmp = typed_project();
    configure_relink_extensions(tmp.path(), &["colon-line"]);
    let literal = tmp
        .path()
        .join("work/202402_sa2a7_task/sub/CURRENT_STATE.md:33");
    fs::create_dir_all(literal.parent().unwrap()).unwrap();
    fs::write(&literal, "literal colon filename\n").unwrap();
    assert!(!literal.with_file_name("CURRENT_STATE.md").exists());
    let source = tmp.path().join("knowledge/colon-line-literal.md");
    fs::write(
        &source,
        "[literal](../old/202402_sa2a7_old/sub/CURRENT_STATE.md:33)\n",
    )
    .unwrap();

    let output = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert!(value["findings"].as_array().unwrap().is_empty());
    assert_eq!(
        value["changes"][0]["to"],
        "../work/202402_sa2a7_task/sub/CURRENT_STATE.md:33"
    );
}

#[test]
fn relink_never_scans_or_changes_inbox_note_or_done_queues() {
    let tmp = typed_project();
    let paths = [
        tmp.path().join("work/202402_sa2a7_task/inbox/message.md"),
        tmp.path()
            .join("work/202402_sa2a7_task/inbox/done/filed.md"),
        tmp.path().join("scratch/notes/pending.md"),
        tmp.path().join("scratch/notes/quarantine/suspect.md"),
        tmp.path().join("scratch/notes/done/filed.md"),
        tmp.path().join("knowledge/tmp/ignored.md"),
        tmp.path().join("knowledge/.git/config.md"),
        tmp.path().join("knowledge/ignored-by-rule.md"),
    ];
    fs::write(
        tmp.path().join("knowledge/.gitignore"),
        "ignored-by-rule.md\n",
    )
    .unwrap();
    for path in &paths {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "[hidden](../old/202402_sa2a7_old/CURRENT_STATE.md)\n").unwrap();
    }
    let before = paths
        .iter()
        .map(|path| (path.clone(), fs::read(path).unwrap()))
        .collect::<Vec<_>>();
    let output = sid()
        .args(["relink", "--write"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert!(value["changes"].as_array().unwrap().iter().all(|change| {
        let path = change["path"].as_str().unwrap();
        !path.contains("/inbox/") && !path.contains("/notes/")
    }));
    for (path, bytes) in before {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}

#[test]
fn relink_candidate_findings_do_not_make_a_usable_scan_partial() {
    let tmp = typed_project();
    let source = tmp.path().join("knowledge/findings.md");
    fs::write(
        &source,
        "[missing](../old/202402_sc4c9_gone/CURRENT_STATE.md)\n\
[suffix](../old/202402_sa2a7_old/missing.txt)\n",
    )
    .unwrap();
    let output = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["complete"], true);
    assert!(value["changes"].as_array().unwrap().is_empty());
    let codes = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"relink-unresolved-ref"));
    assert!(codes.contains(&"relink-missing-internal-target"));
}

#[test]
fn relink_normalizes_absolute_paths_but_skips_correct_external_and_fragment_links() {
    let tmp = typed_project();
    let source = tmp.path().join("knowledge/normalization.md");
    let target = tmp.path().join("work/202402_sa2a7_task/CURRENT_STATE.md");
    fs::write(
        &source,
        format!(
            "[absolute]({})\n[correct](../work/202402_sa2a7_task/CURRENT_STATE.md)\n[external](https://example.test/202402_sa2a7_old)\n[fragment](#202402_sa2a7_old)\n",
            target.display()
        ),
    )
    .unwrap();
    let output = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        value["changes"][0]["to"],
        "../work/202402_sa2a7_task/CURRENT_STATE.md"
    );
}

#[test]
fn relink_reports_ambiguous_refs_and_invalid_utf8_as_distinct_coverage_states() {
    let tmp = typed_project();
    write_canonical(
        &tmp.path()
            .join("work/202405_sa2a7_duplicate/CURRENT_STATE.md"),
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"Duplicate\"\ntimestamp: \"2024-05-01\"\n",
        "",
    );
    fs::write(
        tmp.path().join("knowledge/ambiguous.md"),
        "[ambiguous](../old/202402_sa2a7_old/CURRENT_STATE.md)\n",
    )
    .unwrap();
    fs::write(tmp.path().join("knowledge/binary.md"), [0xff, 0xfe]).unwrap();
    let output = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["complete"], false);
    let codes = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"relink-ambiguous-ref"));
    assert!(codes.contains(&"unreadable-entry"));
    assert_eq!(
        fs::read(tmp.path().join("knowledge/binary.md")).unwrap(),
        [0xff, 0xfe]
    );
}

#[test]
fn relink_redirects_an_old_seed_file_to_its_graduated_task_entrypoint() {
    let tmp = typed_project();
    fs::remove_file(tmp.path().join("parking/202403_sb3b8_parked.md")).unwrap();
    write_canonical(
        &tmp.path()
            .join("work/202403_sb3b8_graduated/CURRENT_STATE.md"),
        "type: \"task\"\nid: \"sb3b8\"\ntitle: \"Graduated\"\ntimestamp: \"2024-03-01\"\norigin: [\"sa2a7\"]\n",
        "## Related Work\n- sa2a7\n",
    );
    let source = tmp.path().join("knowledge/graduated.md");
    fs::write(&source, "[old seed](../parking/202403_sb3b8_parked.md)\n").unwrap();
    let output = sid()
        .args(["relink", "--write"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["changes"].as_array().unwrap().len(), 1);
    assert!(
        fs::read_to_string(source)
            .unwrap()
            .contains("../work/202403_sb3b8_graduated/CURRENT_STATE.md")
    );
}

#[test]
fn relink_projected_preview_repairs_inbound_and_outbound_without_writing() {
    let tmp = move_project();
    let inbound = tmp.path().join("knowledge/inbound.md");
    let outbound = tmp.path().join("work/202402_sa2a7_task/notes.md");
    write_markdown(
        &inbound,
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    write_markdown(&outbound, "[b](../202404_sd5d2_other/CURRENT_STATE.md)\n");
    let before = [text_of(&inbound), text_of(&outbound)];
    let before_mtimes = [mtime_of(&inbound), mtime_of(&outbound)];

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(
        keys(&preview),
        [
            "applied",
            "changes",
            "complete",
            "findings",
            "plan_sha256",
            "projection"
        ]
    );
    assert_eq!(
        keys(&preview["projection"]),
        ["from_owner", "id", "settled", "to_owner"]
    );
    let base = tmp.path().canonicalize().unwrap();
    assert_eq!(preview["projection"]["id"], "sa2a7");
    assert_eq!(
        preview["projection"]["from_owner"].as_str().unwrap(),
        base.join("work/202402_sa2a7_task").to_str().unwrap()
    );
    assert_eq!(
        preview["projection"]["to_owner"].as_str().unwrap(),
        base.join("work/.archive/202402_sa2a7_task")
            .to_str()
            .unwrap()
    );
    assert_eq!(preview["projection"]["settled"], false);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["complete"], true);
    assert!(finding_codes(&preview).is_empty());
    plan_digest(&preview);
    assert_eq!(
        change_pairs(&preview),
        [
            (
                "../work/202402_sa2a7_task/CURRENT_STATE.md".to_string(),
                "../work/.archive/202402_sa2a7_task/CURRENT_STATE.md".to_string()
            ),
            (
                "../202404_sd5d2_other/CURRENT_STATE.md".to_string(),
                "../../202404_sd5d2_other/CURRENT_STATE.md".to_string()
            ),
        ]
    );
    assert_eq!([text_of(&inbound), text_of(&outbound)], before);
    assert_eq!([mtime_of(&inbound), mtime_of(&outbound)], before_mtimes);
}

#[test]
fn relink_projected_repairs_ref_less_moved_source_destinations() {
    let tmp = move_project();
    write_markdown(&tmp.path().join("GUIDE.md"), "# Guide\n");
    write_markdown(
        &tmp.path()
            .join("shared/202402_sa2a7_task/202404_sd5d2_other.md"),
        "# Multi ref\n",
    );
    write_markdown(
        &tmp.path().join("work/202402_sa2a7_task/assets/plain.md"),
        "# Plain\n",
    );
    let source = tmp.path().join("work/202402_sa2a7_task/links.md");
    write_markdown(
        &source,
        "[guide](../../GUIDE.md)\n\
[multi](../../shared/202402_sa2a7_task/202404_sd5d2_other.md)\n\
[together](assets/plain.md)\n\
[external](https://example.test/a)\n\
[fragment](#local)\n",
    );

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(preview["complete"], true, "{preview:#}");
    assert!(finding_codes(&preview).is_empty(), "{preview:#}");
    assert_eq!(
        change_pairs(&preview),
        [
            (
                "../../GUIDE.md".to_string(),
                "../../../GUIDE.md".to_string()
            ),
            (
                "../../shared/202402_sa2a7_task/202404_sd5d2_other.md".to_string(),
                "../../../shared/202402_sa2a7_task/202404_sd5d2_other.md".to_string(),
            ),
        ]
    );
    for change in preview["changes"].as_array().unwrap() {
        assert_eq!(keys(change), ["column", "from", "id", "line", "path", "to"]);
        assert!(change["id"].is_null(), "{change:#}");
    }

    let applied = projected_write(tmp.path(), "sa2a7", ".archive", &plan_digest(&preview));
    assert_eq!(applied["complete"], true, "{applied:#}");
    fs::create_dir_all(tmp.path().join("work/.archive")).unwrap();
    fs::rename(
        tmp.path().join("work/202402_sa2a7_task"),
        tmp.path().join("work/.archive/202402_sa2a7_task"),
    )
    .unwrap();
    let moved = tmp.path().join("work/.archive/202402_sa2a7_task/links.md");
    let text = text_of(&moved);
    assert!(text.contains("[guide](../../../GUIDE.md)"));
    assert!(text.contains("[multi](../../../shared/202402_sa2a7_task/202404_sd5d2_other.md)"));
    assert!(text.contains("[together](assets/plain.md)"));
    assert!(moved.parent().unwrap().join("../../../GUIDE.md").exists());
    assert!(
        moved
            .parent()
            .unwrap()
            .join("../../../shared/202402_sa2a7_task/202404_sd5d2_other.md")
            .exists()
    );
    assert!(moved.parent().unwrap().join("assets/plain.md").exists());
}

#[test]
fn relink_projected_generic_destination_retry_is_forward_only() {
    let tmp = move_project();
    write_markdown(&tmp.path().join("GUIDE.md"), "# Guide\n");
    let source = tmp.path().join("work/202402_sa2a7_task/links.md");
    write_markdown(&source, "[guide](../../../GUIDE.md)\n");

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(preview["complete"], true, "{preview:#}");
    assert!(preview["changes"].as_array().unwrap().is_empty());
    assert!(finding_codes(&preview).is_empty());
    assert_eq!(
        text_of(&source),
        "[guide](../../../GUIDE.md)\n",
        "future-correct text must never be reversed before rename"
    );
}

#[test]
fn relink_projected_generic_destination_ambiguity_and_missing_target_block() {
    let ambiguous = move_project();
    write_markdown(&ambiguous.path().join("GUIDE.md"), "# Current\n");
    write_markdown(&ambiguous.path().join("work/GUIDE.md"), "# Projected\n");
    let ambiguous_source = ambiguous.path().join("work/202402_sa2a7_task/links.md");
    write_markdown(&ambiguous_source, "[guide](../../GUIDE.md)\n");
    let ambiguous_preview = projected_preview(ambiguous.path(), "sa2a7", ".archive");
    assert_eq!(ambiguous_preview["complete"], false);
    assert!(ambiguous_preview["changes"].as_array().unwrap().is_empty());
    assert!(
        finding_codes(&ambiguous_preview).contains(&"relink-projection-drift"),
        "{ambiguous_preview:#}"
    );

    let missing = move_project();
    let missing_source = missing.path().join("work/202402_sa2a7_task/links.md");
    write_markdown(&missing_source, "[missing](../../MISSING.md)\n");
    let missing_preview = projected_preview(missing.path(), "sa2a7", ".archive");
    assert_eq!(missing_preview["complete"], false);
    assert!(missing_preview["changes"].as_array().unwrap().is_empty());
    assert!(
        finding_codes(&missing_preview).contains(&"relink-projection-drift"),
        "{missing_preview:#}"
    );
}

#[test]
fn relink_projected_v2_digest_binds_generic_effect_source_bytes() {
    let tmp = move_project();
    write_markdown(&tmp.path().join("GUIDE.md"), "# Guide\n");
    let generic = tmp.path().join("work/202402_sa2a7_task/links.md");
    write_markdown(&generic, "[guide](../../GUIDE.md)\n");
    let irrelevant = tmp.path().join("knowledge/external.md");
    write_markdown(
        &irrelevant,
        "[external](https://example.test/a)\n[fragment](#local)\n",
    );

    let first = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(first["complete"], true, "{first:#}");
    assert_eq!(first["changes"].as_array().unwrap().len(), 1);
    assert!(first["changes"][0]["id"].is_null());
    let baseline = plan_digest(&first);
    let shape = change_pairs(&first);

    write_markdown(
        &generic,
        "[guide](../../GUIDE.md)\n\nGeneric effect prose.\n",
    );
    let changed = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(change_pairs(&changed), shape);
    assert_ne!(plan_digest(&changed), baseline);

    let generic_digest = plan_digest(&changed);
    write_markdown(
        &irrelevant,
        "[external](https://example.test/a)\n[fragment](#local)\n\nIrrelevant prose.\n",
    );
    assert_eq!(
        plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive")),
        generic_digest,
        "external and fragment-only destinations must remain outside v2 authority"
    );
}

#[test]
fn relink_projected_write_requires_matching_digest_before_any_file() {
    let tmp = move_project();
    let inbound = tmp.path().join("knowledge/inbound.md");
    write_markdown(
        &inbound,
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let stale = plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive"));

    let added = tmp.path().join("knowledge/added.md");
    write_markdown(&added, "[b](../work/202402_sa2a7_task/CURRENT_STATE.md)\n");
    let before = [text_of(&inbound), text_of(&added)];

    let refused = projected_write(tmp.path(), "sa2a7", ".archive", &stale);
    assert_eq!(refused["applied"], false);
    assert_eq!(refused["complete"], false);
    assert!(refused["changes"].as_array().unwrap().is_empty());
    assert!(finding_codes(&refused).contains(&"relink-plan-changed"));
    let current = plan_digest(&refused);
    assert_ne!(current, stale);
    assert_eq!([text_of(&inbound), text_of(&added)], before);

    let fresh = projected_preview(tmp.path(), "sa2a7", ".archive");
    let fresh_digest = plan_digest(&fresh);
    assert_eq!(
        fresh_digest, current,
        "a refusal must return the newly computed plan digest"
    );
    let applied = projected_write(tmp.path(), "sa2a7", ".archive", &fresh_digest);
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["complete"], true);
    assert_eq!(applied["changes"].as_array().unwrap().len(), 2);
    for path in [&inbound, &added] {
        assert!(
            text_of(path).contains("../work/.archive/202402_sa2a7_task/CURRENT_STATE.md"),
            "{}",
            path.display()
        );
    }
}

#[test]
fn relink_projected_preview_is_stable_and_ignores_unrelated_authored_edits() {
    let tmp = move_project();
    write_markdown(
        &tmp.path().join("knowledge/inbound.md"),
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let unrelated = tmp.path().join("knowledge/unrelated.md");
    write_markdown(
        &unrelated,
        "[c](../work/202404_sd5d2_other/CURRENT_STATE.md)\n",
    );

    let first = plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive"));
    let repeated = plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive"));
    assert_eq!(first, repeated, "identical scoped state must be stable");

    write_markdown(
        &unrelated,
        "[c](../work/202404_sd5d2_other/CURRENT_STATE.md)\n\nUnrelated prose.\n",
    );
    assert_eq!(
        plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive")),
        first,
        "an edit outside the move effect set must not invalidate approval"
    );

    write_markdown(
        &tmp.path().join("knowledge/new-inbound.md"),
        "[d](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let with_inbound = plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive"));
    assert_ne!(with_inbound, first, "a new inbound link must invalidate");

    write_markdown(
        &tmp.path().join("work/202402_sa2a7_task/outbound.md"),
        "[e](../202404_sd5d2_other/CURRENT_STATE.md)\n",
    );
    assert_ne!(
        plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive")),
        with_inbound,
        "a new relevant outbound link must invalidate"
    );
}

#[test]
fn relink_projected_drift_blocks_write() {
    let tmp = move_project();
    let drifted = tmp.path().join("knowledge/drifted.md");
    write_markdown(
        &drifted,
        "[a](../work/202402_sa2a7_wrong/CURRENT_STATE.md)\n",
    );
    let before = text_of(&drifted);

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(preview["complete"], false);
    assert_eq!(preview["applied"], false);
    assert!(preview["changes"].as_array().unwrap().is_empty());
    assert!(finding_codes(&preview).contains(&"relink-projection-drift"));
    let digest = plan_digest(&preview);

    let refused = projected_write(tmp.path(), "sa2a7", ".archive", &digest);
    assert_eq!(refused["applied"], false);
    assert_eq!(refused["complete"], false);
    assert!(refused["changes"].as_array().unwrap().is_empty());
    assert!(finding_codes(&refused).contains(&"relink-projection-drift"));
    assert!(
        !finding_codes(&refused).contains(&"relink-plan-changed"),
        "an incomplete plan refuses on its own, not as a digest mismatch"
    );
    assert_eq!(text_of(&drifted), before);
}

#[test]
fn relink_projected_retry_treats_future_destinations_as_settled() {
    let tmp = move_project();
    let settled = tmp.path().join("knowledge/settled.md");
    let current = tmp.path().join("knowledge/current.md");
    write_markdown(
        &settled,
        "[a](../work/.archive/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let settled_before = text_of(&settled);
    write_markdown(
        &current,
        "[b](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(preview["complete"], true);
    assert_eq!(
        change_pairs(&preview),
        [(
            "../work/202402_sa2a7_task/CURRENT_STATE.md".to_string(),
            "../work/.archive/202402_sa2a7_task/CURRENT_STATE.md".to_string()
        )]
    );

    let applied = projected_write(tmp.path(), "sa2a7", ".archive", &plan_digest(&preview));
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["complete"], true);
    assert_eq!(
        text_of(&settled),
        settled_before,
        "an already-future destination must never be reversed"
    );

    let convergent = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(convergent["complete"], true);
    assert!(convergent["changes"].as_array().unwrap().is_empty());

    let converged = projected_write(tmp.path(), "sa2a7", ".archive", &plan_digest(&convergent));
    assert_eq!(converged["applied"], true);
    assert_eq!(converged["complete"], true);
    assert!(converged["changes"].as_array().unwrap().is_empty());
    assert_eq!(text_of(&settled), settled_before);
}

#[test]
fn relink_projected_settled_owner_verifies_scoped_destinations() {
    let tmp = move_project();
    // The owner already lives under the destination root, as it does when close
    // verification or a retry runs after the lifecycle rename already happened.
    fs::create_dir_all(tmp.path().join("work/.archive")).unwrap();
    fs::rename(
        tmp.path().join("work/202402_sa2a7_task"),
        tmp.path().join("work/.archive/202402_sa2a7_task"),
    )
    .unwrap();
    let inbound = tmp.path().join("knowledge/inbound.md");
    write_markdown(
        &inbound,
        "[a](../work/.archive/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );

    let settled = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(settled["projection"]["settled"], true);
    assert_eq!(
        settled["projection"]["from_owner"],
        settled["projection"]["to_owner"]
    );
    assert_eq!(settled["complete"], true);
    assert_eq!(settled["applied"], false);
    assert!(settled["changes"].as_array().unwrap().is_empty());
    assert!(finding_codes(&settled).is_empty());

    write_markdown(
        &inbound,
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let drifted = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(drifted["projection"]["settled"], true);
    assert_eq!(drifted["complete"], false);
    assert!(
        drifted["changes"].as_array().unwrap().is_empty(),
        "settled verification never plans a move-caused change"
    );
    assert!(finding_codes(&drifted).contains(&"relink-projection-drift"));
}

#[test]
fn relink_projected_review_move_and_suffixes_preserve_contract() {
    let tmp = move_project();
    configure_relink_extensions(tmp.path(), &["colon-line"]);
    let source = tmp.path().join("knowledge/review.md");
    write_markdown(
        &source,
        "[r](../prs/202401_816d_review/CURRENT_STATE.md#part)\n\
[n](<../prs/202401_816d_review/notes.md>)\n\
[l](../prs/202401_816d_review/notes.md:33)\n",
    );

    let preview = projected_preview(tmp.path(), "816d", ".archive-prs");
    assert_eq!(preview["complete"], true);
    assert_eq!(
        change_pairs(&preview),
        [
            (
                "../prs/202401_816d_review/CURRENT_STATE.md#part".to_string(),
                "../prs/.archive-prs/202401_816d_review/CURRENT_STATE.md#part".to_string()
            ),
            (
                "../prs/202401_816d_review/notes.md".to_string(),
                "../prs/.archive-prs/202401_816d_review/notes.md".to_string()
            ),
            (
                "../prs/202401_816d_review/notes.md:33".to_string(),
                "../prs/.archive-prs/202401_816d_review/notes.md:33".to_string()
            ),
        ]
    );

    let applied = projected_write(tmp.path(), "816d", ".archive-prs", &plan_digest(&preview));
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["complete"], true);
    let text = text_of(&source);
    assert!(text.contains("[r](../prs/.archive-prs/202401_816d_review/CURRENT_STATE.md#part)"));
    assert!(text.contains("[n](<../prs/.archive-prs/202401_816d_review/notes.md>)"));
    assert!(text.contains("[l](../prs/.archive-prs/202401_816d_review/notes.md:33)"));
}

#[test]
fn relink_projected_self_link_with_unchanged_relative_path_is_not_an_effect() {
    let tmp = move_project();
    let deep = tmp.path().join("work/202402_sa2a7_task/sub/deep.md");
    write_markdown(&deep, "[self](../../202402_sa2a7_task/CURRENT_STATE.md)\n");
    let before = text_of(&deep);

    let projected = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(projected["complete"], true);
    assert!(
        projected["changes"].as_array().unwrap().is_empty(),
        "source and target move together, so the move causes no change"
    );
    assert!(finding_codes(&projected).is_empty());

    // Global relink still owns ordinary normalization of the same destination,
    // so the projected omission is scoping, not a lost repair.
    let global = sid()
        .arg("relink")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let global = json(&global);
    assert_eq!(
        keys(&global),
        ["applied", "changes", "complete", "findings"]
    );
    assert_eq!(
        change_pairs(&global),
        [(
            "../../202402_sa2a7_task/CURRENT_STATE.md".to_string(),
            "../CURRENT_STATE.md".to_string()
        )]
    );
    assert_eq!(text_of(&deep), before);
}

#[cfg(unix)]
#[test]
fn relink_projected_partial_write_is_forward_recoverable() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = move_project();
    let writable = tmp.path().join("knowledge/writable.md");
    let locked_dir = tmp.path().join("knowledge/locked");
    let locked = locked_dir.join("locked.md");
    write_markdown(
        &writable,
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    write_markdown(
        &locked,
        "[b](../../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    let digest = plan_digest(&preview);
    assert_eq!(preview["changes"].as_array().unwrap().len(), 2);

    // An unwritable parent directory makes the atomic replacement of exactly
    // one planned file fail after the whole-plan digest already matched.
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o555)).unwrap();
    let partial = projected_write(tmp.path(), "sa2a7", ".archive", &digest);
    fs::set_permissions(&locked_dir, fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(partial["applied"], true);
    assert_eq!(partial["complete"], false);
    assert_eq!(partial["changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        plan_digest(&partial),
        digest,
        "a projected write returns the approved pre-write digest"
    );
    assert!(finding_codes(&partial).contains(&"relink-write-failed"));
    assert!(text_of(&writable).contains("../work/.archive/202402_sa2a7_task/CURRENT_STATE.md"));
    assert!(text_of(&locked).contains("../../work/202402_sa2a7_task/CURRENT_STATE.md"));

    let retry = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(retry["complete"], true);
    assert_eq!(
        change_pairs(&retry),
        [(
            "../../work/202402_sa2a7_task/CURRENT_STATE.md".to_string(),
            "../../work/.archive/202402_sa2a7_task/CURRENT_STATE.md".to_string()
        )],
        "a fresh preview plans only the remaining repair"
    );
}

#[test]
fn relink_projected_depth_sensitive_authored_destination_is_drift_not_unchanged() {
    let tmp = move_project();
    // Authored from four levels below the owner, walking above the owner and
    // re-descending. It resolves correctly today, and both the current and the
    // projected canonical spellings are the identical `../../notes.md`, so
    // comparing canonical text alone would call this unaffected. The move
    // changes the owner's depth, so the authored bytes break.
    let deep = tmp.path().join("work/202402_sa2a7_task/a/b/deep.md");
    write_markdown(&deep, "[x](../../../../work/202402_sa2a7_task/notes.md)\n");
    write_markdown(&tmp.path().join("work/202402_sa2a7_task/notes.md"), "# N\n");
    let before = text_of(&deep);
    assert!(
        tmp.path()
            .join("work/202402_sa2a7_task/a/b/../../../../work/202402_sa2a7_task/notes.md")
            .exists(),
        "fixture must resolve before the move or it proves nothing"
    );

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(
        preview["complete"], false,
        "a destination that breaks under the move must never report complete"
    );
    assert!(preview["changes"].as_array().unwrap().is_empty());
    assert!(finding_codes(&preview).contains(&"relink-projection-drift"));

    let refused = projected_write(tmp.path(), "sa2a7", ".archive", &plan_digest(&preview));
    assert_eq!(refused["applied"], false);
    assert_eq!(text_of(&deep), before);

    // The benign twin still settles: same relative relationship, authored path
    // does not cross the owner boundary, so the move cannot affect it.
    let benign = move_project();
    let sub = benign.path().join("work/202402_sa2a7_task/sub/deep.md");
    write_markdown(&sub, "[self](../../202402_sa2a7_task/CURRENT_STATE.md)\n");
    let benign_preview = projected_preview(benign.path(), "sa2a7", ".archive");
    assert_eq!(benign_preview["complete"], true);
    assert!(benign_preview["changes"].as_array().unwrap().is_empty());
    assert!(finding_codes(&benign_preview).is_empty());
}

#[cfg(unix)]
#[test]
fn relink_projected_refuses_a_dangling_symlink_at_the_destination() {
    let tmp = move_project();
    fs::create_dir_all(tmp.path().join("work/.archive")).unwrap();
    // `exists()` follows symlinks and reports false here, so a guard built on it
    // would let the scoped write through and leave the caller's rename to fail.
    std::os::unix::fs::symlink(
        tmp.path().join("work/does-not-exist"),
        tmp.path().join("work/.archive/202402_sa2a7_task"),
    )
    .unwrap();
    assert!(
        !tmp.path().join("work/.archive/202402_sa2a7_task").exists(),
        "fixture must be a dangling symlink for this test to bite"
    );
    let inbound = tmp.path().join("knowledge/inbound.md");
    write_markdown(
        &inbound,
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let before = text_of(&inbound);

    let output = sid()
        .args(["relink", "--move", "sa2a7", "--into", ".archive"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
    assert_eq!(text_of(&inbound), before);
}

#[test]
fn relink_projected_digest_binds_effect_source_bytes_without_a_plan_change() {
    let tmp = move_project();
    // One planned change plus one file that is in the effect set but needs no
    // replacement, so the settled arm of effect-set classification is covered.
    let planned = tmp.path().join("knowledge/planned.md");
    let settled = tmp.path().join("knowledge/settled.md");
    write_markdown(
        &planned,
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    write_markdown(
        &settled,
        "[b](../work/.archive/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );

    let first = projected_preview(tmp.path(), "sa2a7", ".archive");
    let baseline = plan_digest(&first);
    let plan_shape = (change_pairs(&first), finding_codes(&first).len());

    // Appending prose to a file that carries a relevant link changes no planned
    // change and no finding, so only the effect-source byte hashes can notice.
    write_markdown(
        &planned,
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n\nLater prose.\n",
    );
    let after_planned_edit = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(
        (
            change_pairs(&after_planned_edit),
            finding_codes(&after_planned_edit).len()
        ),
        plan_shape,
        "the visible plan must be identical, or this proves nothing about bytes"
    );
    assert_ne!(
        plan_digest(&after_planned_edit),
        baseline,
        "editing an effect source must invalidate approval even with no plan delta"
    );

    // Same again for the settled effect source, which contributes no change at
    // all and so is only reachable through the in-effect-set classification.
    let restored = move_project();
    write_markdown(
        &restored.path().join("knowledge/planned.md"),
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let settled_only = restored.path().join("knowledge/settled.md");
    write_markdown(
        &settled_only,
        "[b](../work/.archive/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let before_settled_edit = plan_digest(&projected_preview(restored.path(), "sa2a7", ".archive"));
    write_markdown(
        &settled_only,
        "[b](../work/.archive/202402_sa2a7_task/CURRENT_STATE.md)\n\nLater prose.\n",
    );
    let after = projected_preview(restored.path(), "sa2a7", ".archive");
    assert_eq!(
        change_pairs(&after).len(),
        1,
        "the settled file still contributes no change"
    );
    assert_ne!(
        plan_digest(&after),
        before_settled_edit,
        "a settled in-effect-set file must also bind the digest"
    );
}

#[test]
fn relink_projected_stale_digest_reports_plan_changed_even_when_incomplete() {
    let tmp = move_project();
    let inbound = tmp.path().join("knowledge/inbound.md");
    write_markdown(
        &inbound,
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let stale = plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive"));

    // Now make the plan both different *and* incomplete. The caller must still
    // learn its approval was stale; reporting only incompleteness would hide
    // that a fresh preview is required.
    write_markdown(
        &tmp.path().join("knowledge/drifted.md"),
        "[b](../work/202402_sa2a7_wrong/CURRENT_STATE.md)\n",
    );
    let before = text_of(&inbound);

    let refused = projected_write(tmp.path(), "sa2a7", ".archive", &stale);
    assert_eq!(refused["applied"], false);
    assert_eq!(refused["complete"], false);
    assert!(refused["changes"].as_array().unwrap().is_empty());
    let codes = finding_codes(&refused);
    assert!(
        codes.contains(&"relink-plan-changed"),
        "stale approval must be reported before incompleteness: {codes:?}"
    );
    assert!(codes.contains(&"relink-projection-drift"));
    assert_ne!(plan_digest(&refused), stale);
    assert_eq!(text_of(&inbound), before);
}

#[test]
fn relink_projected_owner_boundary_excludes_a_name_prefixed_sibling() {
    let tmp = move_project();
    // `202402_sa2a7_task-notes` starts with the owner's folder name as a string
    // but is a different directory. A string-prefix boundary would wrongly treat
    // its files as moving and rewrite their outbound links.
    let sibling = tmp
        .path()
        .join("work/202402_sa2a7_task-notes/CURRENT_STATE.md");
    write_canonical(
        &sibling,
        "type: \"task\"\nid: \"se6e3\"\ntitle: \"Sibling\"\ntimestamp: \"2024-06-01\"\n",
        "",
    );
    let sibling_link = tmp.path().join("work/202402_sa2a7_task-notes/link.md");
    write_markdown(
        &sibling_link,
        "[b](../202404_sd5d2_other/CURRENT_STATE.md)\n",
    );
    let before = text_of(&sibling_link);

    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    assert_eq!(preview["complete"], true);
    assert!(
        preview["changes"].as_array().unwrap().is_empty(),
        "a name-prefixed sibling does not move: {:?}",
        change_pairs(&preview)
    );
    let digest = plan_digest(&preview);

    // The observable consequence of a wrong owner boundary is digest scope, not
    // the change set: `project_path` is component-correct on its own, so a
    // string-prefix boundary still plans nothing here — it just wrongly pulls the
    // sibling's bytes into the approval and makes unrelated edits invalidate it.
    write_markdown(
        &sibling_link,
        "[b](../202404_sd5d2_other/CURRENT_STATE.md)\n\nSibling prose.\n",
    );
    assert_eq!(
        plan_digest(&projected_preview(tmp.path(), "sa2a7", ".archive")),
        digest,
        "a name-prefixed sibling is outside the effect set and must not bind the digest"
    );

    let applied = projected_write(tmp.path(), "sa2a7", ".archive", &digest);
    assert_eq!(applied["applied"], true);
    assert!(text_of(&sibling_link).starts_with(&before));
}

#[test]
fn relink_projected_write_returns_exactly_the_projected_key_set() {
    let tmp = move_project();
    write_markdown(
        &tmp.path().join("knowledge/inbound.md"),
        "[a](../work/202402_sa2a7_task/CURRENT_STATE.md)\n",
    );
    let preview = projected_preview(tmp.path(), "sa2a7", ".archive");
    let applied = projected_write(tmp.path(), "sa2a7", ".archive", &plan_digest(&preview));
    assert_eq!(
        keys(&applied),
        [
            "applied",
            "changes",
            "complete",
            "findings",
            "plan_sha256",
            "projection"
        ]
    );
    assert_eq!(
        keys(&applied["projection"]),
        ["from_owner", "id", "settled", "to_owner"]
    );
}

#[test]
fn relink_projected_rejects_invalid_identity_root_and_digest_arguments() {
    let tmp = move_project();
    let hex = "0".repeat(64);
    let short = "a".repeat(63);
    let long = "a".repeat(65);
    let upper = "A".repeat(64);
    let not_hex = "g".repeat(64);
    let cases: Vec<Vec<&str>> = vec![
        vec!["relink", "--move", "sa2a7"],
        vec!["relink", "--into", ".archive"],
        vec!["relink", "--move", "sa2a7", "--into", ".nope"],
        vec!["relink", "--move", "sb3b8", "--into", ".archive"],
        vec!["relink", "--move", "topic/guide", "--into", ".archive"],
        vec!["relink", "--move", "zzzz9", "--into", ".archive"],
        vec!["relink", "--move", "sa2a7", "--into", ".archive", "--write"],
        vec!["relink", "--write", "--expected-plan-sha256", &hex],
        vec![
            "relink",
            "--move",
            "sa2a7",
            "--into",
            ".archive",
            "--expected-plan-sha256",
            &hex,
        ],
        vec![
            "relink",
            "--move",
            "sa2a7",
            "--into",
            ".archive",
            "--write",
            "--expected-plan-sha256",
            &short,
        ],
        vec![
            "relink",
            "--move",
            "sa2a7",
            "--into",
            ".archive",
            "--write",
            "--expected-plan-sha256",
            &long,
        ],
        vec![
            "relink",
            "--move",
            "sa2a7",
            "--into",
            ".archive",
            "--write",
            "--expected-plan-sha256",
            &upper,
        ],
        vec![
            "relink",
            "--move",
            "sa2a7",
            "--into",
            ".archive",
            "--write",
            "--expected-plan-sha256",
            &not_hex,
        ],
    ];
    for args in cases {
        let output = sid()
            .args(&args)
            .current_dir(tmp.path())
            .assert()
            .failure()
            .get_output()
            .clone();
        assert!(
            output.stdout.is_empty(),
            "stdout must stay empty for {args:?}"
        );
    }

    // The far side of the digest boundary: exactly 64 lowercase hex characters
    // is a well-formed argument, so it reaches the plan comparison and returns
    // a usable exit-0 refusal instead of an argument failure.
    let refused = projected_write(tmp.path(), "sa2a7", ".archive", &hex);
    assert_eq!(refused["applied"], false);
    assert!(finding_codes(&refused).contains(&"relink-plan-changed"));
}

#[test]
fn relink_projected_refuses_ambiguous_root_duplicate_id_and_existing_destination() {
    let ambiguous = tempfile::tempdir().unwrap();
    fs::write(
        ambiguous.path().join(".sid"),
        "[task]\nroot = \"work\"\nscan_roots = [\"work/.archive\", \"prs/.archive\"]\n",
    )
    .unwrap();
    write_canonical(
        &ambiguous
            .path()
            .join("work/202402_sa2a7_task/CURRENT_STATE.md"),
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"Task\"\ntimestamp: \"2024-02-01\"\n",
        "",
    );

    let duplicate = move_project();
    write_canonical(
        &duplicate
            .path()
            .join("prs/202405_sa2a7_dup/CURRENT_STATE.md"),
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"Duplicate\"\ntimestamp: \"2024-05-01\"\n",
        "",
    );

    let collision = move_project();
    fs::create_dir_all(collision.path().join("work/.archive/202402_sa2a7_task")).unwrap();

    for project in [ambiguous.path(), duplicate.path(), collision.path()] {
        let output = sid()
            .args(["relink", "--move", "sa2a7", "--into", ".archive"])
            .current_dir(project)
            .assert()
            .failure()
            .get_output()
            .clone();
        assert!(
            output.stdout.is_empty(),
            "stdout must stay empty for {}",
            project.display()
        );
    }
}
