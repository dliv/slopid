use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

fn sid() -> assert_cmd::Command {
    cargo_bin_cmd!("sid")
}

fn run(dir: &Path, args: &[&str]) -> Output {
    sid().args(args).current_dir(dir).output().unwrap()
}

fn run_bounded(dir: &Path, args: &[&str], timeout: Duration) -> Output {
    let child = Command::new(env!("CARGO_BIN_EXE_sid"))
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_bounded(child, timeout, args)
}

fn wait_bounded(mut child: Child, timeout: Duration, args: &[&str]) -> Output {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().unwrap();
            panic!(
                "sid {args:?} exceeded {timeout:?}; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap()
}

fn assert_keys(value: &Value, expected: &[&str]) {
    let mut actual = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

fn assert_error(output: &Output, expected_code: &str, expected_status: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(expected_status),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let value = json(&output.stderr);
    assert_keys(&value, &["error"]);
    let error_keys = value["error"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert!(
        error_keys == ["code", "message"] || error_keys == ["candidates", "code", "message"],
        "{value}"
    );
    assert_eq!(value["error"]["code"], expected_code, "{value}");
    assert!(
        value["error"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
    );
    value
}

fn write_config(dir: &Path, contents: &str) {
    fs::write(dir.join(".sid"), contents).unwrap();
}

fn canonical_root(dir: &Path, relative: &str) -> PathBuf {
    dir.canonicalize().unwrap().join(relative)
}

fn create_folder(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

fn set_mtime(path: &Path, when: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

#[cfg(unix)]
fn make_fifo(path: &Path) {
    let status = Command::new("mkfifo").arg(path).status().unwrap();
    assert!(status.success(), "mkfifo failed for {}", path.display());
}

#[test]
fn help_and_version_advertise_only_the_identity_surface() {
    let help = sid()
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(help).unwrap();
    for command in [
        "init",
        "root",
        "new",
        "list",
        "resolve",
        "lint",
        "agent-instructions",
    ] {
        assert!(help.contains(command), "missing {command}: {help}");
    }
    for removed in [
        "graph", "search", "context", "captures", "note", "seed", "relink",
    ] {
        assert!(
            !help.contains(removed),
            "advertised removed command {removed}: {help}"
        );
    }
    let new_help = sid()
        .args(["new", "--help"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let new_help = String::from_utf8(new_help).unwrap();
    assert!(!new_help.contains("--period"));
    assert!(!new_help.contains("--into"));
    assert!(!new_help.contains("--from-seed"));
}

#[test]
fn init_writes_only_the_flat_default_and_refuses_overwrite() {
    let tmp = tempfile::tempdir().unwrap();
    let output = run(tmp.path(), &["init"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = json(&output.stdout);
    assert_keys(&result, &["created", "path"]);
    assert_eq!(result["created"], true);
    assert_eq!(
        PathBuf::from(result["path"].as_str().unwrap()),
        tmp.path().canonicalize().unwrap().join(".sid")
    );
    assert_eq!(
        fs::read_to_string(tmp.path().join(".sid")).unwrap(),
        "root = \"stm\"\n"
    );
    assert!(!tmp.path().join("stm").exists());
    assert_error(&run(tmp.path(), &["init"]), "sid-init", 1);
    assert_eq!(
        fs::read_to_string(tmp.path().join(".sid")).unwrap(),
        "root = \"stm\"\n"
    );
}

#[test]
fn root_returns_absolute_base_and_logical_root_without_creating_it() {
    let tmp = tempfile::tempdir().unwrap();
    let output = run(tmp.path(), &["root"]);
    assert!(output.status.success());
    let result = json(&output.stdout);
    assert_keys(&result, &["base", "root"]);
    assert_eq!(
        PathBuf::from(result["base"].as_str().unwrap()),
        tmp.path().canonicalize().unwrap()
    );
    assert_eq!(
        PathBuf::from(result["root"].as_str().unwrap()),
        canonical_root(tmp.path(), "stm")
    );
    assert!(!tmp.path().join("stm").exists());

    let nested = tmp.path().join("one/two");
    fs::create_dir_all(&nested).unwrap();
    write_config(tmp.path(), "root = \"work/tasks\"\n");
    let nearest = json(&run(&nested, &["root"]).stdout);
    assert_keys(&nearest, &["base", "root"]);
    assert_eq!(
        PathBuf::from(nearest["base"].as_str().unwrap()),
        tmp.path().canonicalize().unwrap()
    );
    assert_eq!(
        PathBuf::from(nearest["root"].as_str().unwrap()),
        canonical_root(tmp.path(), "work/tasks")
    );
    let human = run(&nested, &["root", "--human"]);
    assert!(human.status.success());
    assert_eq!(
        String::from_utf8(human.stdout).unwrap(),
        format!("{}\n", canonical_root(tmp.path(), "work/tasks").display())
    );
    assert!(!tmp.path().join("work/tasks").exists());
}

#[test]
fn config_rejects_old_tables_unknown_fields_and_unsafe_roots_as_sid_config() {
    let configs = [
        "[task]\nroot = \"stm\"\n",
        "root = \"stm\"\nunknown = true\n",
        "root = \"/tmp/outside\"\n",
        "root = \"../outside\"\n",
        "root = \"\"\n",
        "root = \".\"\n",
        "[ref]\nprefixes = []\n",
    ];
    for config in configs {
        let tmp = tempfile::tempdir().unwrap();
        write_config(tmp.path(), config);
        assert_error(&run(tmp.path(), &["root"]), "sid-config", 1);
        assert!(!tmp.path().join("stm").exists(), "{config}");
    }
}

#[cfg(unix)]
#[test]
fn dangling_config_symlink_fails_closed_with_sid_config() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    symlink(tmp.path().join("missing"), tmp.path().join(".sid")).unwrap();
    assert_error(&run(tmp.path(), &["root"]), "sid-config", 1);
    assert!(!tmp.path().join("stm").exists());
}

#[test]
fn new_preserves_its_exact_result_and_dry_run_has_no_write() {
    let tmp = tempfile::tempdir().unwrap();
    write_config(tmp.path(), "root = \"tasks\"\n");
    let dry = run(
        tmp.path(),
        &["new", "fix auth state", "--dry-run", "--period", "202605"],
    );
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let plan = json(&dry.stdout);
    assert_keys(
        &plan,
        &[
            "title", "slug", "period", "sid_ref", "id", "path", "dry_run",
        ],
    );
    assert_eq!(plan["title"], "fix auth state");
    assert_eq!(plan["slug"], "fix-auth-state");
    assert_eq!(plan["period"], "202605");
    assert_eq!(plan["dry_run"], true);
    assert_eq!(
        PathBuf::from(plan["path"].as_str().unwrap())
            .parent()
            .unwrap(),
        canonical_root(tmp.path(), "tasks")
    );
    assert!(!tmp.path().join("tasks").exists());

    let created = run(tmp.path(), &["new", "fix auth state", "--period", "202605"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let result = json(&created.stdout);
    assert_keys(
        &result,
        &[
            "title", "slug", "period", "sid_ref", "id", "path", "dry_run",
        ],
    );
    assert_eq!(result["dry_run"], false);
    assert!(Path::new(result["path"].as_str().unwrap()).is_dir());
}

#[test]
fn allocation_uses_default_replacement_and_empty_deny_prefix_policies() {
    let default = tempfile::tempdir().unwrap();
    create_folder(&default.path().join("stm"), "202605_sew2a_existing");
    let result = json(
        &run(
            default.path(),
            &["new", "default policy", "--dry-run", "--period", "202605"],
        )
        .stdout,
    );
    assert!(result["sid_ref"].as_str().unwrap().starts_with("sey"));

    let replacement = tempfile::tempdir().unwrap();
    write_config(replacement.path(), "[ref]\ndeny_prefixes = [\"sey\"]\n");
    create_folder(&replacement.path().join("stm"), "202605_sew2a_existing");
    let result = json(
        &run(
            replacement.path(),
            &[
                "new",
                "replacement policy",
                "--dry-run",
                "--period",
                "202605",
            ],
        )
        .stdout,
    );
    assert!(result["sid_ref"].as_str().unwrap().starts_with("sex"));

    let empty = tempfile::tempdir().unwrap();
    write_config(empty.path(), "[ref]\ndeny_prefixes = []\n");
    create_folder(&empty.path().join("stm"), "202605_sew2a_existing");
    let result = json(
        &run(
            empty.path(),
            &["new", "empty policy", "--dry-run", "--period", "202605"],
        )
        .stdout,
    );
    assert!(result["sid_ref"].as_str().unwrap().starts_with("sex"));

    let invalid = tempfile::tempdir().unwrap();
    write_config(invalid.path(), "[ref]\ndeny_prefixes = [\"xx\"]\n");
    assert_error(
        &run(invalid.path(), &["new", "invalid policy", "--dry-run"]),
        "sid-config",
        1,
    );
    assert!(!invalid.path().join("stm").exists());
}

#[test]
fn allocation_accepts_six_digit_non_calendar_periods_and_checks_both_sequence_edges() {
    let arbitrary = tempfile::tempdir().unwrap();
    let result = run(
        arbitrary.path(),
        &[
            "new",
            "period shape only",
            "--dry-run",
            "--period",
            "209913",
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(json(&result.stdout)["period"], "209913");

    let boundary = tempfile::tempdir().unwrap();
    create_folder(&boundary.path().join("stm"), "202605_szy2a_penultimate");
    let final_slot = run(
        boundary.path(),
        &["new", "final sequence", "--period", "202605"],
    );
    assert!(
        final_slot.status.success(),
        "{}",
        String::from_utf8_lossy(&final_slot.stderr)
    );
    assert!(
        json(&final_slot.stdout)["sid_ref"]
            .as_str()
            .unwrap()
            .starts_with("szz")
    );
    assert_error(
        &run(boundary.path(), &["new", "exhausted", "--period", "202605"]),
        "sid-new",
        1,
    );
}

#[test]
fn malformed_period_and_empty_slug_fail_before_writing() {
    for period in ["20261", "2026056", "2026-5", "abcdef"] {
        let tmp = tempfile::tempdir().unwrap();
        assert_error(
            &run(
                tmp.path(),
                &["new", "valid title", "--dry-run", "--period", period],
            ),
            "sid-new",
            1,
        );
        assert!(!tmp.path().join("stm").exists());
    }
    let tmp = tempfile::tempdir().unwrap();
    assert_error(
        &run(tmp.path(), &["new", "🚀‼️", "--dry-run"]),
        "sid-new",
        1,
    );
    assert!(!tmp.path().join("stm").exists());
}

#[test]
fn nearest_nested_config_wins_and_invalid_nearest_config_fails_before_write() {
    let tmp = tempfile::tempdir().unwrap();
    write_config(tmp.path(), "root = \"outer-tasks\"\n");
    let inner = tmp.path().join("inner");
    fs::create_dir_all(&inner).unwrap();
    write_config(&inner, "root = \"inner-tasks\"\n");
    let result = run(&inner, &["new", "nearest wins", "--period", "202605"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let path = PathBuf::from(json(&result.stdout)["path"].as_str().unwrap());
    assert_eq!(
        path.parent().unwrap(),
        canonical_root(&inner, "inner-tasks")
    );
    assert!(path.is_dir());
    assert!(!tmp.path().join("outer-tasks").exists());

    let broken = tempfile::tempdir().unwrap();
    write_config(broken.path(), "root = \"outer-tasks\"\n");
    let nested = broken.path().join("nested");
    fs::create_dir_all(&nested).unwrap();
    write_config(&nested, "unknown = true\n");
    assert_error(
        &run(&nested, &["new", "must not write", "--dry-run"]),
        "sid-config",
        1,
    );
    assert!(!broken.path().join("outer-tasks").exists());
    assert!(!nested.join("stm").exists());
}

#[test]
fn allocation_keeps_a_final_sequence_file_as_a_reservation() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    fs::create_dir_all(&root).unwrap();
    create_folder(&root, "202605_szy2a_penultimate");
    fs::write(root.join("202605_szz2a_reserved"), "reservation").unwrap();
    let output = run(
        tmp.path(),
        &["new", "must not reuse final sequence", "--period", "202605"],
    );
    assert_error(&output, "sid-new", 1);
    assert_eq!(
        fs::read_to_string(root.join("202605_szz2a_reserved")).unwrap(),
        "reservation"
    );
}

#[cfg(unix)]
#[test]
fn allocation_keeps_a_final_sequence_symlink_as_a_reservation() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    fs::create_dir_all(&root).unwrap();
    create_folder(&root, "202605_szy2a_penultimate");
    symlink(
        tmp.path().join("missing-target"),
        root.join("202605_szz2a_reserved"),
    )
    .unwrap();
    assert_error(
        &run(
            tmp.path(),
            &["new", "must not reuse final sequence", "--period", "202605"],
        ),
        "sid-new",
        1,
    );
}

#[test]
fn list_returns_only_real_direct_identity_folders_with_exact_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    fs::create_dir_all(&root).unwrap();
    create_folder(&root, "202610_sa2a7_identity");
    create_folder(&root, "202611_sb3b8");
    fs::write(root.join("202612_sc4c9_export.txt"), "reserved").unwrap();
    create_folder(&root.join("202610_sa2a7_identity"), "202613_sf7f4_nested");
    create_folder(&root, "202615_sdmr192_invalid");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let external = tmp.path().join("external");
        fs::create_dir(&external).unwrap();
        symlink(&external, root.join("202613_sd5d2_directory-link")).unwrap();
        symlink(
            tmp.path().join("missing"),
            root.join("202614_se6e3_broken-link"),
        )
        .unwrap();
    }

    let output = run(tmp.path(), &["list", "--sort", "id"]);
    assert!(output.status.success());
    let result = json(&output.stdout);
    assert_keys(&result, &["folders"]);
    assert!(result.get("tasks").is_none());
    let folders = result["folders"].as_array().unwrap();
    let ids = folders
        .iter()
        .map(|folder| folder["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["202610_sa2a7_identity", "202611_sb3b8"]);
    for folder in folders {
        assert_keys(
            folder,
            &[
                "id", "period", "sid_ref", "slug", "modified", "root", "path",
            ],
        );
    }
    assert_eq!(folders[0]["period"], "202610");
    assert_eq!(folders[0]["sid_ref"], "sa2a7");
    assert_eq!(folders[0]["slug"], "identity");
    assert!(chrono::DateTime::parse_from_rfc3339(folders[0]["modified"].as_str().unwrap()).is_ok());
    assert_eq!(
        PathBuf::from(folders[0]["root"].as_str().unwrap()),
        canonical_root(tmp.path(), "stm")
    );
    assert_eq!(
        PathBuf::from(folders[0]["path"].as_str().unwrap()),
        canonical_root(tmp.path(), "stm/202610_sa2a7_identity")
    );
}

#[test]
fn list_missing_root_is_empty_and_terms_are_case_insensitive_and_anded() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = json(&run(tmp.path(), &["list"]).stdout);
    assert_eq!(missing, serde_json::json!({"folders": []}));
    assert!(!tmp.path().join("stm").exists());
    let root = tmp.path().join("stm");
    fs::create_dir_all(&root).unwrap();
    create_folder(&root, "202610_sa2a7_historical-delivery-triage");
    create_folder(&root, "202611_sb3b8_java-review");
    let matching = json(&run(tmp.path(), &["list", "DELIVERY", "Triage"]).stdout);
    assert_eq!(matching["folders"].as_array().unwrap().len(), 1);
    assert_eq!(matching["folders"][0]["sid_ref"], "sa2a7");
    let ref_prefix = json(&run(tmp.path(), &["list", "sb"]).stdout);
    assert_eq!(ref_prefix["folders"].as_array().unwrap().len(), 1);
}

#[test]
fn list_human_output_is_plain_and_does_not_change_machine_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    create_folder(&root, "202610_sa2a7_alpha");
    let output = run(tmp.path(), &["list", "--human"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(!text.contains('{'), "{text}");
    assert!(text.contains("202610_sa2a7_alpha"), "{text}");
    assert!(text.trim_end().ends_with("stm"), "{text}");
    assert!(
        !text
            .lines()
            .next()
            .unwrap()
            .split("  ")
            .next()
            .unwrap()
            .contains('T')
    );

    let broken = tempfile::tempdir().unwrap();
    fs::write(broken.path().join("stm"), "not a directory").unwrap();
    assert_error(&run(broken.path(), &["list", "--human"]), "sid-list", 1);
}

#[cfg(unix)]
#[test]
fn recency_uses_direct_child_metadata_without_following_child_symlinks() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    fs::create_dir_all(&root).unwrap();
    let first = create_folder(&root, "202610_sa2a7_first");
    let second = create_folder(&root, "202611_sb3b8_second");
    let now = SystemTime::now();
    let direct = first.join("direct.txt");
    fs::write(&direct, "direct").unwrap();
    set_mtime(&direct, now + Duration::from_secs(2 * 3600));
    let deep = first.join("child/deep.txt");
    fs::create_dir_all(deep.parent().unwrap()).unwrap();
    fs::write(&deep, "deep").unwrap();
    set_mtime(&deep, now + Duration::from_secs(8 * 3600));
    let target = tmp.path().join("target.txt");
    fs::write(&target, "target").unwrap();
    set_mtime(&target, now + Duration::from_secs(12 * 3600));
    symlink(&target, first.join("linked.txt")).unwrap();
    let other_direct = second.join("direct.txt");
    fs::write(&other_direct, "direct").unwrap();
    set_mtime(&other_direct, now + Duration::from_secs(4 * 3600));

    let output = run(tmp.path(), &["list"]);
    assert!(output.status.success());
    let result = json(&output.stdout);
    let ids = result["folders"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["202611_sb3b8_second", "202610_sa2a7_first"]);
}

#[test]
fn resolve_uses_folder_names_without_entrypoints_or_markdown_parsing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    let no_entrypoint = create_folder(&root, "202610_sa2a7_no-entrypoint");
    let output = run(tmp.path(), &["resolve", "sa2a7"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result = json(&output.stdout);
    assert_keys(&result, &["folder"]);
    assert_keys(
        &result["folder"],
        &["id", "period", "sid_ref", "slug", "root", "path"],
    );
    assert_eq!(result["folder"]["id"], "202610_sa2a7_no-entrypoint");
    assert_eq!(
        PathBuf::from(result["folder"]["path"].as_str().unwrap()),
        canonical_root(tmp.path(), "stm/202610_sa2a7_no-entrypoint")
    );
    fs::write(
        no_entrypoint.join("CURRENT_STATE.md"),
        "not markdown frontmatter",
    )
    .unwrap();
    assert!(
        run_bounded(tmp.path(), &["resolve", "sa2a7"], Duration::from_secs(2))
            .status
            .success()
    );
}

#[cfg(unix)]
#[test]
fn resolve_list_lint_and_root_do_not_open_current_state_fifo() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    let folder = create_folder(&root, "202610_sa2a7_fifo");
    make_fifo(&folder.join("CURRENT_STATE.md"));
    for args in [
        vec!["resolve", "sa2a7"],
        vec!["list"],
        vec!["lint"],
        vec!["root"],
    ] {
        let output = run_bounded(tmp.path(), &args, Duration::from_secs(2));
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn resolve_short_ref_ambiguity_is_sorted_and_full_name_recovers() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    create_folder(&root, "202611_sa2a7_zulu");
    create_folder(&root, "202610_sa2a7_alpha");
    let ambiguous = assert_error(
        &run(tmp.path(), &["resolve", "sa2a7"]),
        "sid-resolve-ambiguous",
        1,
    );
    assert_keys(&ambiguous["error"], &["code", "message", "candidates"]);
    let candidates = ambiguous["error"]["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0]["id"], "202610_sa2a7_alpha");
    assert_eq!(candidates[1]["id"], "202611_sa2a7_zulu");
    for candidate in candidates {
        assert_keys(candidate, &["id", "path"]);
    }
    let exact = run(tmp.path(), &["resolve", "202611_sa2a7_zulu"]);
    assert!(
        exact.status.success(),
        "{}",
        String::from_utf8_lossy(&exact.stderr)
    );
    assert_eq!(json(&exact.stdout)["folder"]["id"], "202611_sa2a7_zulu");
}

#[test]
fn file_reservations_do_not_make_folder_resolution_ambiguous_but_lint_sees_them() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    create_folder(&root, "202610_sa2a7_real");
    fs::write(root.join("202611_sa2a7_export.zip"), "reserved").unwrap();
    let resolved = run(tmp.path(), &["resolve", "sa2a7"]);
    assert!(
        resolved.status.success(),
        "{}",
        String::from_utf8_lossy(&resolved.stderr)
    );
    assert_eq!(json(&resolved.stdout)["folder"]["id"], "202610_sa2a7_real");
    let lint = run(tmp.path(), &["lint"]);
    assert_eq!(lint.status.code(), Some(1));
    assert_eq!(
        json(&lint.stdout)["findings"][0]["code"],
        "identity-ref-duplicate"
    );
}

#[test]
fn resolve_distinguishes_invalid_and_absent_selectors() {
    let tmp = tempfile::tempdir().unwrap();
    create_folder(&tmp.path().join("stm"), "202610_sa2a7_present");
    for selector in [
        "../name",
        "/tmp/202610_sa2a7_present",
        "SA2A7",
        "sa2",
        "202610_sa2a7_slug/child",
        "202610_sa2a7_slug/",
    ] {
        assert_error(
            &run(tmp.path(), &["resolve", selector]),
            "sid-resolve-invalid",
            1,
        );
    }
    assert_error(
        &run(tmp.path(), &["resolve", "szz2a"]),
        "sid-resolve-not-found",
        1,
    );
}

#[test]
fn list_and_resolve_fail_closed_when_root_is_not_a_directory() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("stm"), "not a directory").unwrap();
    assert_error(&run(tmp.path(), &["list"]), "sid-list", 1);
    assert_error(
        &run(tmp.path(), &["resolve", "sa2a7"]),
        "sid-resolve-unavailable",
        1,
    );
}

#[cfg(unix)]
#[test]
fn configured_root_symlink_is_followed_and_dangling_root_scans_empty() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let external = tmp.path().join("external");
    create_folder(&external, "202610_sa2a7_linked-root");
    symlink(&external, tmp.path().join("stm")).unwrap();
    let result = json(&run(tmp.path(), &["list"]).stdout);
    assert_eq!(result["folders"].as_array().unwrap().len(), 1);
    assert_eq!(
        PathBuf::from(result["folders"][0]["root"].as_str().unwrap()),
        canonical_root(tmp.path(), "stm")
    );

    let dangling = tempfile::tempdir().unwrap();
    symlink(dangling.path().join("missing"), dangling.path().join("stm")).unwrap();
    assert_eq!(
        json(&run(dangling.path(), &["list"]).stdout),
        serde_json::json!({"folders": []})
    );
    let created = run(
        dangling.path(),
        &[
            "new",
            "cannot create through dangling root",
            "--period",
            "202605",
        ],
    );
    assert_error(&created, "sid-new", 1);
}

#[test]
fn config_and_selector_error_precedence_and_clap_usage_are_stable() {
    let tmp = tempfile::tempdir().unwrap();
    write_config(tmp.path(), "not = [valid");
    assert_error(
        &run(tmp.path(), &["resolve", "../invalid"]),
        "sid-config",
        1,
    );
    assert_error(&run(tmp.path(), &["resolve"]), "sid-usage", 2);
    assert_error(&run(tmp.path(), &["new"]), "sid-usage", 2);
    assert_error(
        &run(tmp.path(), &["new", "title", "--into", "stm"]),
        "sid-usage",
        2,
    );
    assert_error(
        &run(tmp.path(), &["new", "--from-seed", "sa2a7"]),
        "sid-usage",
        2,
    );
    assert_error(&run(tmp.path(), &["graph", "sa2a7"]), "sid-usage", 2);
    assert_error(
        &run(tmp.path(), &["--json", "new", "title"]),
        "sid-usage",
        2,
    );
}

#[test]
fn help_version_and_agent_instructions_bypass_broken_project_config() {
    let tmp = tempfile::tempdir().unwrap();
    write_config(tmp.path(), "not = [valid");
    for args in [&["--help"][..], &["--version"][..]] {
        let output = run(tmp.path(), args);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert!(!output.stdout.is_empty());
        assert!(serde_json::from_slice::<Value>(&output.stdout).is_err());
    }
    let instructions = run(tmp.path(), &["agent-instructions"]);
    assert!(instructions.status.success());
    let value = json(&instructions.stdout);
    assert!(value["text"].as_str().unwrap().contains("folders"));
    assert!(!value["text"].as_str().unwrap().contains("graph"));
}

#[test]
fn lint_keeps_identity_findings_and_report_exit_semantics() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("stm");
    create_folder(&root, "202610_sa2a7_one");
    fs::write(root.join("202611_sa2a7_export.zip"), "reservation").unwrap();
    create_folder(&root, "202612_sdmr192_invalid");
    let lint = run(tmp.path(), &["lint"]);
    assert_eq!(lint.status.code(), Some(1));
    let report = json(&lint.stdout);
    assert_keys(&report, &["complete", "healthy", "findings"]);
    assert_eq!(report["complete"], true);
    assert_eq!(report["healthy"], false);
    let findings = report["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0]["code"], "identity-folder-ref-invalid");
    assert_eq!(findings[1]["code"], "identity-ref-duplicate");
    assert!(
        findings[1]["message"]
            .as_str()
            .unwrap()
            .contains("202611_sa2a7_export.zip")
    );

    let config_error = tempfile::tempdir().unwrap();
    write_config(config_error.path(), "unknown = true\n");
    assert_error(&run(config_error.path(), &["lint"]), "sid-config", 1);

    let incomplete = tempfile::tempdir().unwrap();
    fs::write(incomplete.path().join("stm"), "not a directory").unwrap();
    let report = run(incomplete.path(), &["lint"]);
    assert_eq!(report.status.code(), Some(2));
    assert_eq!(json(&report.stdout)["complete"], false);
    assert!(report.stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn init_config_error_is_distinct_for_dangling_symlink() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    symlink(tmp.path().join("missing"), tmp.path().join(".sid")).unwrap();
    assert_error(&run(tmp.path(), &["init"]), "sid-init", 1);
}

#[cfg(unix)]
#[test]
fn init_does_not_misreport_resolvable_or_looping_config_symlinks_as_dangling() {
    use std::os::unix::fs::symlink;

    let resolvable = tempfile::tempdir().unwrap();
    let target = resolvable.path().join("real-config");
    fs::write(&target, "root = \"elsewhere\"\n").unwrap();
    symlink(&target, resolvable.path().join(".sid")).unwrap();
    let failure = assert_error(&run(resolvable.path(), &["init"]), "sid-init", 1);
    assert!(
        failure["error"]["message"]
            .as_str()
            .unwrap()
            .contains("already exists")
    );
    assert!(
        !failure["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dangling")
    );
    assert_eq!(
        fs::read_to_string(target).unwrap(),
        "root = \"elsewhere\"\n"
    );

    let looping = tempfile::tempdir().unwrap();
    symlink(".sid", looping.path().join(".sid")).unwrap();
    let failure = assert_error(&run(looping.path(), &["init"]), "sid-init", 1);
    assert!(
        failure["error"]["message"]
            .as_str()
            .unwrap()
            .contains("already exists")
    );
    assert!(
        !failure["error"]["message"]
            .as_str()
            .unwrap()
            .contains("dangling")
    );
}
