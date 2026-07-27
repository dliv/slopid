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
        "applied",
        "[ref].deny_prefixes",
        "Apart from generated-prefix selection",
        "`sid list` reader semantics are unchanged",
    ] {
        assert!(text.contains(phrase), "missing {phrase}");
    }
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
