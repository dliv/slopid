use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::fs;
use std::path::Path;

fn sid() -> assert_cmd::Command {
    cargo_bin_cmd!("sid")
}

fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join(".sid"),
        "[task]\nroot = \"stm\"\nscan_roots = [\"stm/.archive\"]\n",
    )
    .unwrap();
    tmp
}

fn entry(base: &Path, root: &str, folder: &str, frontmatter: &str, body: &str) {
    let dir = base.join(root).join(folder);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("CURRENT_STATE.md"),
        format!("---\n{frontmatter}---\n{body}"),
    )
    .unwrap();
}

fn seed_graph(base: &Path) {
    entry(
        base,
        "stm/.archive",
        "202401_816d_old",
        "type: \"task\"\nid: \"816d\"\ntitle: \"Old\"\ntimestamp: \"2024-01-01\"\nextension: []\n",
        "",
    );
    entry(
        base,
        "stm",
        "202402_sa2a7_middle",
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"Middle\"\ntimestamp: \"2024-02-01\"\norigin: [\"816d\"]\nrelated: [\"sb3b8\"]\n",
        "## Related STMs\n- 816d and sb3b8\n",
    );
    entry(
        base,
        "stm",
        "202403_sb3b8_new",
        "type: \"task\"\nid: \"sb3b8\"\ntitle: \"New\"\ntimestamp: \"2024-03-01\"\nsupersedes: [\"sa2a7\"]\n",
        "## Related work\n- sa2a7\n",
    );
}

fn json(output: &[u8]) -> Value {
    serde_json::from_slice(output).unwrap()
}

#[test]
fn resolve_returns_exact_canonical_node_and_preserves_extension_values() {
    let tmp = project();
    seed_graph(tmp.path());
    let output = sid()
        .args(["resolve", "816d"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(
        value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["node"]
    );
    assert_eq!(value["node"].as_object().unwrap().len(), 2);
    assert_eq!(value["node"]["frontmatter"]["id"], "816d");
    assert_eq!(
        value["node"]["frontmatter"]["extension"],
        serde_json::json!([])
    );
    assert!(value["node"]["path"].as_str().unwrap().starts_with('/'));
}

#[test]
fn resolve_is_exact_case_sensitive_and_failure_has_empty_stdout() {
    let tmp = project();
    seed_graph(tmp.path());
    for id in ["SA2A7", "sa2", "202402_sa2a7_middle", "missing"] {
        let output = sid()
            .args(["resolve", id])
            .current_dir(tmp.path())
            .assert()
            .failure()
            .get_output()
            .clone();
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn graph_walks_backlinks_related_both_ways_and_orders_oldest_first() {
    let tmp = project();
    seed_graph(tmp.path());
    let output = sid()
        .args(["graph", "816d"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, ["anchor", "complete", "edges", "findings", "nodes"]);
    assert_eq!(value["anchor"], "816d");
    assert_eq!(value["complete"], true);
    assert_eq!(
        value["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["frontmatter"]["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["816d", "sa2a7", "sb3b8"]
    );
    assert_eq!(value["edges"].as_array().unwrap().len(), 3);
}

#[test]
fn graph_filters_depth_direction_and_edges_without_changing_completeness() {
    let tmp = project();
    seed_graph(tmp.path());
    fs::create_dir_all(tmp.path().join("stm/202404_sc4c9_missing")).unwrap();
    for args in [
        vec!["graph", "sa2a7", "--depth", "0"],
        vec!["graph", "sa2a7", "--direction", "incoming"],
        vec![
            "graph",
            "sa2a7",
            "--direction",
            "outgoing",
            "--edge",
            "origin",
        ],
    ] {
        let output = sid()
            .args(args)
            .current_dir(tmp.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let value = json(&output);
        assert_eq!(value["complete"], false);
        assert!(
            value["findings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|finding| finding["code"] == "missing-entrypoint")
        );
    }
}

#[test]
fn graph_collapses_duplicate_edges_omits_self_edges_and_terminates_cycles() {
    let tmp = project();
    entry(
        tmp.path(),
        "stm",
        "202401_sa2a7_one",
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"One\"\ntimestamp: \"2024-01-01\"\nrelated: [\"sb3b8\", \"sb3b8\", \"sa2a7\"]\n",
        "## Related\n- sb3b8\n",
    );
    entry(
        tmp.path(),
        "stm",
        "202402_sb3b8_two",
        "type: \"task\"\nid: \"sb3b8\"\ntitle: \"Two\"\ntimestamp: \"2024-02-01\"\nrelated: [\"sa2a7\"]\n",
        "## Related\n- sa2a7\n",
    );
    let output = sid()
        .args(["graph", "sa2a7"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["complete"], true);
    assert_eq!(value["edges"].as_array().unwrap().len(), 2);
    assert!(value["findings"].as_array().unwrap().is_empty());
}

/// Every entry type that reserves a ref is valid identity: folders, files, and
/// symlinks alike. Names that do not parse as `{YYYYMM}_{ref}` — dot entries,
/// invalid periods, and arbitrary names — are ignored, and a configured root
/// that does not exist yet is an empty namespace rather than a defect.
#[test]
fn lint_reports_a_clean_identity_namespace_and_exits_zero() {
    let tmp = project();
    fs::create_dir_all(tmp.path().join("stm/202401_sa2a7_folder")).unwrap();
    fs::create_dir_all(tmp.path().join("stm/.archive")).unwrap();
    fs::write(tmp.path().join("stm/202402_sb3b8_export.zip"), "reserved").unwrap();
    std::os::unix::fs::symlink("../nowhere", tmp.path().join("stm/202403_sc4c9_moved-away"))
        .unwrap();
    fs::create_dir_all(tmp.path().join("stm/2026_sd5d2_bad-period")).unwrap();
    fs::create_dir_all(tmp.path().join("stm/.hidden")).unwrap();
    fs::write(tmp.path().join("stm/README.md"), "ignored").unwrap();

    let output = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        json(&output),
        serde_json::json!({
            "complete": true,
            "healthy": true,
            "findings": []
        })
    );
}

/// An unrecognized ref under a valid period and one ref reserved twice across
/// two configured allocation roots are fully observed namespace defects: the
/// report stays complete, turns unhealthy, and exits one.
#[test]
fn lint_reports_invalid_and_duplicate_refs_as_complete_errors() {
    let tmp = project();
    fs::create_dir_all(tmp.path().join("stm/202401_sa2a7_one")).unwrap();
    fs::create_dir_all(tmp.path().join("stm/.archive/202312_sa2a7_older")).unwrap();
    fs::create_dir_all(tmp.path().join("stm/202405_816d_legacy")).unwrap();

    let output = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["complete"], true);
    assert_eq!(value["healthy"], false);
    let findings = value["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 2, "{findings:?}");
    assert_eq!(findings[0]["code"], "identity-folder-ref-invalid");
    assert_eq!(findings[0]["severity"], "error");
    assert_eq!(findings[0]["ref_id"], "816d");
    assert_eq!(findings[0]["path"], "stm/202405_816d_legacy");
    assert_eq!(findings[1]["code"], "identity-ref-duplicate");
    assert_eq!(findings[1]["ref_id"], "sa2a7");
    assert_eq!(findings[1]["path"], serde_json::Value::Null);
    let duplicate = findings[1]["message"].as_str().unwrap();
    assert!(
        duplicate.contains("stm/.archive/202312_sa2a7_older"),
        "{duplicate}"
    );
    assert!(duplicate.contains("stm/202401_sa2a7_one"), "{duplicate}");
    for finding in findings {
        let path = finding["path"].as_str().unwrap_or_default();
        assert!(!path.starts_with('/'), "{path} must be config-relative");
        assert!(
            !finding["message"]
                .as_str()
                .unwrap()
                .contains(&format!("{}", tmp.path().display())),
            "message leaked an absolute path"
        );
    }
}

/// An unreadable configured root is a coverage failure, not a data verdict:
/// the partial report is still emitted and the process exits two.
#[test]
fn lint_reports_an_unreadable_root_as_an_incomplete_report() {
    let tmp = project();
    fs::create_dir_all(tmp.path().join("stm")).unwrap();
    fs::write(tmp.path().join("stm/.archive"), "not a directory").unwrap();

    let output = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(value["complete"], false);
    let findings = value["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0]["code"], "identity-root-unreadable");
    assert_eq!(findings[0]["path"], "stm/.archive");
}

/// The executable proof that identity lint opens no authored Markdown: a
/// canonical folder whose entrypoint is not even UTF-8, an invalid inbox
/// envelope, a capture note, and a topic document all stay healthy while the
/// folder names remain unique.
#[test]
fn lint_ignores_authored_documents_and_non_allocation_roots() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(
        tmp.path().join(".sid"),
        "[task]\nroot = \"stm\"\nscan_roots = [\"stm/.archive\"]\n[note]\nroot = \"stm/.notes\"\n[topic]\nroots = [\"knowledge\"]\n",
    )
    .unwrap();
    let folder = tmp.path().join("stm/202401_sa2a7_task");
    fs::create_dir_all(folder.join("inbox")).unwrap();
    fs::write(folder.join("CURRENT_STATE.md"), [0xff, 0xfe, 0x00, 0x6e]).unwrap();
    fs::write(folder.join("inbox/bad.md"), "not an envelope at all").unwrap();
    fs::create_dir_all(tmp.path().join("stm/.notes")).unwrap();
    fs::write(
        tmp.path().join("stm/.notes/202401_note.md"),
        "---\nbroken\n",
    )
    .unwrap();
    fs::create_dir_all(tmp.path().join("knowledge")).unwrap();
    fs::write(tmp.path().join("knowledge/guide.md"), "---\nbroken\n").unwrap();

    let output = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        json(&output),
        serde_json::json!({
            "complete": true,
            "healthy": true,
            "findings": []
        })
    );
}

#[test]
fn help_describes_reader_commands_and_json_contract() {
    let root = sid()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let root = String::from_utf8(root).unwrap();
    for word in ["resolve", "graph", "lint", "JSON"] {
        assert!(root.contains(word), "root help omitted {word}");
    }
    for command in ["resolve", "graph", "lint"] {
        let output = sid()
            .args([command, "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert!(String::from_utf8(output).unwrap().contains("JSON"));
    }
}

#[test]
fn agent_instructions_teach_exact_reader_usage() {
    let output = sid()
        .arg("agent-instructions")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    let text = value["text"].as_str().unwrap();
    for example in [
        "sid root",
        "sid resolve se2vv",
        "sid graph sdz85",
        "sid lint",
        "read-only",
        "canonical",
    ] {
        assert!(
            text.contains(example),
            "agent instructions omitted {example}"
        );
    }
}
