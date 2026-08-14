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

#[test]
fn lint_clean_scan_exits_zero_with_exact_json() {
    let tmp = project();
    seed_graph(tmp.path());
    let output = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value = json(&output);
    assert_eq!(
        value.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["findings"]
    );
    assert_eq!(value["findings"], serde_json::json!([]));
}

#[test]
fn lint_data_errors_exit_one_with_complete_json() {
    let tmp = project();
    entry(
        tmp.path(),
        "stm",
        "202401_sa2a7_bad",
        "type: \"task\"\nid: \"sa2a7\"\ntitle: \"Bad\"\ntimestamp: \"not-a-date\"\nstatus: \"ACTIVE\"\nrelated: [\"missing\"]\n",
        "",
    );
    let output = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .code(1)
        .get_output()
        .clone();
    let value = json(&output.stdout);
    assert!(
        value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "invalid-required-field")
    );
    assert!(
        value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "forbidden-status")
    );
    assert!(
        value["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["code"] == "dangling-edge")
    );
}

#[test]
fn lint_operational_failure_exits_two_with_empty_stdout() {
    let tmp = project();
    fs::write(tmp.path().join("stm"), "not a directory").unwrap();
    let output = sid()
        .arg("lint")
        .current_dir(tmp.path())
        .assert()
        .code(2)
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("cannot complete lint scan")
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
